use std::ffi::{OsStr, c_void};
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read, Write};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mo_ipc::{Frame, FrameError, read_frame};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GetLastError,
    HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION,
    EqualSid, GetAce, GetAclInformation, GetSecurityDescriptorControl, GetTokenInformation,
    RevertToSelf, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_GROUPS, TOKEN_QUERY, TokenGroups,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeInfo,
    ImpersonateNamedPipeClient, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    WaitNamedPipeW,
};
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, SE_GROUP_LOGON_ID};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetCurrentThread, INFINITE, OpenProcessToken, OpenThreadToken,
    WaitForSingleObject,
};

const PIPE_PREFIX: &str = r"\\.\pipe\LOCAL\Mo.Input.";
const PIPE_BUFFER_BYTES: u32 = (mo_ipc::MAX_FRAME_LEN as u32) * 2;
const SDDL_REVISION_1: u32 = 1;
const CLIENT_PIPE_ACCESS: u32 = 0x0012_019b;
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
const ERROR_FILE_NOT_FOUND_CODE: i32 = 2;
const ERROR_SEM_TIMEOUT_CODE: i32 = 121;
const ERROR_PIPE_BUSY_CODE: i32 = 231;
pub const MAX_PIPE_SLOTS: usize = 16;
const STREAM_IO_TIMEOUT: Duration = Duration::from_secs(2);
const CANCELLATION_DRAIN_MS: u32 = 1000;

/// Deterministic bounded slot family. Slot zero preserves the legacy endpoint.
pub fn pool_slot_address(base: &PipeAddress, slot: usize) -> io::Result<PipeAddress> {
    if slot >= MAX_PIPE_SLOTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pipe slot exceeds pool bound",
        ));
    }
    if slot == 0 {
        return Ok(base.clone());
    }
    let endpoint = base
        .as_str()
        .strip_prefix(PIPE_PREFIX)
        .expect("validated pipe namespace");
    PipeAddress::new(&format!("{endpoint}.s{slot:02}"))
}

/// Fixed independent first-instance pipes: no client is granted the right to
/// create another instance. Bind the entire family before accepting any input.
#[derive(Debug)]
pub struct PipePool {
    listeners: Vec<PipeListener>,
}

impl PipePool {
    pub fn bind(base: PipeAddress, slots: usize) -> io::Result<Self> {
        if slots == 0 || slots > MAX_PIPE_SLOTS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pipe pool size must be 1-16",
            ));
        }
        let mut listeners = Vec::with_capacity(slots);
        for slot in 0..slots {
            listeners.push(PipeListener::bind(pool_slot_address(&base, slot)?)?);
        }
        Ok(Self { listeners })
    }

    pub fn listeners(&self) -> &[PipeListener] {
        &self.listeners
    }
    pub fn into_listeners(self) -> Vec<PipeListener> {
        self.listeners
    }
}

/// A validated local named-pipe address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipeAddress(String);

impl PipeAddress {
    /// Builds an address under the AppContainer-compatible `LOCAL` pipe prefix.
    pub fn new(endpoint: &str) -> io::Result<Self> {
        if endpoint.is_empty()
            || endpoint.len() > 96
            || !endpoint
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pipe endpoint must be 1-96 ASCII letters, digits, dots, dashes, or underscores",
            ));
        }
        Ok(Self(format!("{PIPE_PREFIX}{endpoint}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn wide(&self) -> Vec<u16> {
        wide_null(self.as_str())
    }
}

/// A single-instance listener protected by the creator's logon SID.
pub struct PipeListener {
    address: PipeAddress,
    server: Option<File>,
    logon_sid: SidBytes,
    reusable_active: Arc<AtomicBool>,
}

impl fmt::Debug for PipeListener {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PipeListener")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl PipeListener {
    pub fn bind(address: PipeAddress) -> io::Result<Self> {
        let logon_sid = process_logon_sid()?;
        let security = SecurityDescriptor::for_logon_sid(&logon_sid)?;
        let server = create_server(&address, &security)?;
        Ok(Self {
            address,
            server: Some(server),
            logon_sid,
            reusable_active: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn address(&self) -> &PipeAddress {
        &self.address
    }

    /// One worker owns this retained first-instance handle for its whole
    /// lifetime. The stream gets a duplicate handle to the same kernel instance,
    /// so dropping/erroring a client disconnects it without vacating the name.
    /// Never call this again until the previous AuthenticatedPipe has dropped.
    pub fn accept_reusable_first_frame(
        &mut self,
        timeout: Duration,
    ) -> io::Result<(AuthenticatedPipe, Frame)> {
        if self.reusable_active.swap(true, Ordering::AcqRel) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "previous reusable stream is still active",
            ));
        }
        let lease = ReuseLease(self.reusable_active.clone());
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| io::Error::other("listener is not armed"))?;
        let stream = AuthenticatedPipe {
            file: server.try_clone()?,
            _reuse_lease: Some(lease),
        };
        connect_server(&stream.file)?;
        let frame = read_server_frame(&stream.file, timeout)?;
        authenticate_client_logon_sid(&stream.file, &self.logon_sid)?;
        Ok((stream, frame))
    }

    /// Accepts one client, reads its first bounded frame, then authenticates it.
    ///
    /// This compatibility entry point consumes the listener. Long-running
    /// servers can accept through [`Self::accept_next_first_frame`] and re-arm
    /// the protected first instance after the authenticated stream closes.
    pub fn accept_first_frame(
        mut self,
        first_frame_timeout: Duration,
    ) -> io::Result<(AuthenticatedPipe, Frame)> {
        self.accept_next_first_frame(first_frame_timeout)
    }

    /// Accepts one client from this listener instance.
    pub fn accept_next_first_frame(
        &mut self,
        first_frame_timeout: Duration,
    ) -> io::Result<(AuthenticatedPipe, Frame)> {
        if self.reusable_active.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "previous reusable stream is still active",
            ));
        }
        let server = self
            .server
            .take()
            .expect("listener owns its server instance");
        connect_server(&server)?;
        let frame = read_server_frame(&server, first_frame_timeout)?;
        authenticate_client_logon_sid(&server, &self.logon_sid)?;
        Ok((
            AuthenticatedPipe {
                file: server,
                _reuse_lease: None,
            },
            frame,
        ))
    }

    /// Recreates the protected first instance after the prior stream closed.
    ///
    /// Legacy consuming-accept helper. The concurrent pool instead retains each
    /// independent first instance; neither path grants clients the right to
    /// create another server instance.
    pub fn rearm(&mut self) -> io::Result<()> {
        if self.server.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "named-pipe listener is already armed",
            ));
        }
        let security = SecurityDescriptor::for_logon_sid(&self.logon_sid)?;
        self.server = Some(create_server(&self.address, &security)?);
        Ok(())
    }

    /// Confirms that the kernel marked this listener as rejecting remote clients.
    pub fn rejects_remote_clients(&self) -> io::Result<bool> {
        let mut flags = 0_u32;
        let result = unsafe {
            // SAFETY: `server` owns a valid named-pipe handle and `flags` is writable.
            GetNamedPipeInfo(
                raw_handle(
                    self.server
                        .as_ref()
                        .expect("listener owns its server instance"),
                ),
                &mut flags,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(flags & PIPE_REJECT_REMOTE_CLIENTS != 0)
    }

    /// Audits the live kernel object's DACL, not merely the descriptor input.
    pub fn has_expected_dacl(&self) -> io::Result<bool> {
        let server = self
            .server
            .as_ref()
            .expect("listener owns its server instance");
        audit_dacl(server, &self.logon_sid)
    }
}

/// A server-side stream whose client logon SID has been authenticated.
#[derive(Debug)]
pub struct AuthenticatedPipe {
    file: File,
    _reuse_lease: Option<ReuseLease>,
}

#[derive(Debug)]
struct ReuseLease(Arc<AtomicBool>);

impl Drop for ReuseLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Read for AuthenticatedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        DeadlineIo::new(&self.file, STREAM_IO_TIMEOUT).read(buffer)
    }
}

impl AuthenticatedPipe {
    /// Reads one frame after bounding the time between its first available byte
    /// and complete assembly. An entirely idle connection is not expired.
    pub fn read_frame_after_activity(&mut self, assembly_timeout: Duration) -> io::Result<Frame> {
        let mut first_byte = [0];
        // Only the first byte may wait indefinitely on a normal idle client.
        // No 1 ms polling loop. Once it arrives, header/payload/short reads all
        // share one assembly deadline; decoder validation bounds allocation.
        DeadlineIo {
            file: &self.file,
            deadline: None,
        }
        .read_exact(&mut first_byte)?;
        let mut reader =
            Cursor::new(first_byte).chain(DeadlineIo::new(&self.file, assembly_timeout));
        read_frame(&mut reader).map_err(frame_io_error)
    }

    /// The entire encoded reply (header and payload, including short writes)
    /// shares one deadline. Completion is not an acknowledgement by the peer.
    pub fn write_frame_with_timeout(&mut self, frame: &Frame, timeout: Duration) -> io::Result<()> {
        mo_ipc::write_frame(&mut DeadlineIo::new(&self.file, timeout), frame)
            .map_err(frame_io_error)
    }
}

impl Write for AuthenticatedPipe {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        DeadlineIo::new(&self.file, STREAM_IO_TIMEOUT).write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        // FlushFileBuffers on a pipe waits for the client to read everything.
        // Each overlapped write is already complete; MOIP handles responses,
        // and flush must not add an unbounded peer-consumption wait.
        Ok(())
    }
}

impl Drop for AuthenticatedPipe {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: this is a server pipe handle; disconnect is best-effort at drop.
            DisconnectNamedPipe(raw_handle(&self.file));
        }
    }
}

/// Client connector using identification-only SQOS.
pub struct PipeClient;

impl PipeClient {
    pub fn connect(address: &PipeAddress, timeout: Duration) -> io::Result<File> {
        let address_wide = address.wide();
        let deadline = Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now);
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for the named-pipe Broker",
                ));
            }
            let remaining_ms = u32::try_from(deadline.duration_since(now).as_millis())
                .unwrap_or(u32::MAX)
                .max(1);
            let waited = unsafe {
                // SAFETY: the address is NUL-terminated and remains alive for the call.
                WaitNamedPipeW(address_wide.as_ptr(), remaining_ms)
            };
            if waited == 0 {
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    Some(ERROR_FILE_NOT_FOUND_CODE) => {
                        if Instant::now() >= deadline {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "timed out waiting for the named-pipe Broker",
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Some(ERROR_SEM_TIMEOUT_CODE) => {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "timed out waiting for an available named-pipe Broker instance",
                        ));
                    }
                    _ => return Err(error),
                }
            }

            let handle = unsafe {
                // SAFETY: arguments follow CreateFileW's named-pipe contract. The SQOS
                // flags let the server identify, but not act as, this client.
                CreateFileW(
                    address_wide.as_ptr(),
                    CLIENT_PIPE_ACCESS,
                    0,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                return file_from_handle(handle);
            }
            let error = io::Error::last_os_error();
            if matches!(
                error.raw_os_error(),
                Some(ERROR_FILE_NOT_FOUND_CODE) | Some(ERROR_PIPE_BUSY_CODE)
            ) {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "timed out waiting for the named-pipe Broker",
                    ));
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            return Err(error);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SidBytes(Vec<u8>);

impl SidBytes {
    fn as_ptr(&self) -> *mut c_void {
        self.0.as_ptr().cast_mut().cast()
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: the wrapper is created only for owned, non-pseudo handles.
            CloseHandle(self.0);
        }
    }
}

struct SecurityDescriptor(*mut c_void);

impl SecurityDescriptor {
    fn for_logon_sid(sid: &SidBytes) -> io::Result<Self> {
        let sid_string = sid_to_string(sid)?;
        // 0x12019b grants read/write/attributes/synchronize but deliberately
        // excludes FILE_CREATE_PIPE_INSTANCE (which aliases FILE_APPEND_DATA).
        let sddl = wide_null(&format!(
            "D:P(A;;0x{CLIENT_PIPE_ACCESS:08x};;;{sid_string})"
        ));
        let mut descriptor = null_mut();
        let result = unsafe {
            // SAFETY: SDDL is NUL-terminated; output points to LocalAlloc memory.
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

struct KernelSecurityDescriptor(*mut c_void);

impl Drop for KernelSecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: GetSecurityInfo returns a LocalAlloc security descriptor.
            LocalFree(self.0);
        }
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: ConvertStringSecurityDescriptor... allocates with LocalAlloc.
            LocalFree(self.0);
        }
    }
}

fn create_server(address: &PipeAddress, security: &SecurityDescriptor) -> io::Result<File> {
    let address_wide = address.wide();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let handle = unsafe {
        // SAFETY: pointers remain alive for the call; the descriptor is valid and
        // self-relative; the returned handle is converted to a uniquely owned File.
        CreateNamedPipeW(
            address_wide.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            0,
            &attributes,
        )
    };
    file_from_handle(handle)
}

fn connect_server(server: &File) -> io::Result<()> {
    let event = io_event()?;
    let mut overlapped = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
    };
    let connected = unsafe {
        // SAFETY: server is overlapped; the event and record remain live until
        // the operation completes. Listening has no client-consumption wait.
        ConnectNamedPipe(raw_handle(server), &mut overlapped)
    };
    if connected != 0 {
        return Ok(());
    }
    let error = unsafe { GetLastError() };
    if error == ERROR_PIPE_CONNECTED {
        Ok(())
    } else if error == ERROR_IO_PENDING {
        complete_pending(server, &mut overlapped, INFINITE).map(|_| ())
    } else {
        Err(io::Error::from_raw_os_error(error as i32))
    }
}

/// One outstanding operation at a time, borrowing the handle and sharing a
/// frame-level absolute deadline. The stack record and caller buffer are never
/// released while Windows can still access them.
struct DeadlineIo<'a> {
    file: &'a File,
    deadline: Option<Instant>,
}

impl<'a> DeadlineIo<'a> {
    fn new(file: &'a File, timeout: Duration) -> Self {
        Self {
            file,
            deadline: Some(
                Instant::now()
                    .checked_add(timeout)
                    .unwrap_or_else(Instant::now),
            ),
        }
    }

    fn transfer(&mut self, buffer: *mut u8, len: usize, writing: bool) -> io::Result<usize> {
        if len == 0 {
            return Ok(0);
        }
        self.remaining_ms()?;
        let event = io_event()?;
        let mut overlapped = OVERLAPPED {
            hEvent: event.0,
            ..Default::default()
        };
        let length = u32::try_from(len).unwrap_or(u32::MAX);
        let mut transferred = 0;
        let started = unsafe {
            // SAFETY: callers provide a valid readable/writable slice. This
            // function waits for completion or drains cancellation before
            // returning, retaining the borrowed buffer, event and OVERLAPPED.
            if writing {
                WriteFile(
                    raw_handle(self.file),
                    buffer.cast(),
                    length,
                    &mut transferred,
                    &mut overlapped,
                )
            } else {
                ReadFile(
                    raw_handle(self.file),
                    buffer.cast(),
                    length,
                    &mut transferred,
                    &mut overlapped,
                )
            }
        };
        if started != 0 {
            return Ok(transferred as usize);
        }
        let error = unsafe { GetLastError() };
        if error != ERROR_IO_PENDING {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        // Recompute after event allocation/submission; do not renew the budget
        // for a pending operation or a short write.
        let millis = self.remaining_ms().unwrap_or(0);
        complete_pending(self.file, &mut overlapped, millis).map(|value| value as usize)
    }

    fn remaining_ms(&self) -> io::Result<u32> {
        match self.deadline {
            None => Ok(INFINITE),
            Some(deadline) => {
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .filter(|value| !value.is_zero())
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::TimedOut, "pipe I/O deadline expired")
                    })?;
                Ok(u32::try_from(remaining.as_millis()).unwrap_or(INFINITE - 1))
            }
        }
    }
}

impl Read for DeadlineIo<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.transfer(buffer.as_mut_ptr(), buffer.len(), false)
    }
}

impl Write for DeadlineIo<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.transfer(buffer.as_ptr().cast_mut(), buffer.len(), true)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn read_server_frame(file: &File, timeout: Duration) -> io::Result<Frame> {
    read_frame(&mut DeadlineIo::new(file, timeout)).map_err(frame_io_error)
}

fn frame_io_error(error: FrameError) -> io::Error {
    match error {
        FrameError::Io(error) => error,
        error => io::Error::new(io::ErrorKind::InvalidData, error),
    }
}

fn io_event() -> io::Result<OwnedHandle> {
    let event = unsafe {
        // SAFETY: unnamed, manual-reset, initially nonsignaled owned event.
        CreateEventW(null(), 1, 0, null())
    };
    if event.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(OwnedHandle(event))
    }
}

fn complete_pending(file: &File, overlapped: &mut OVERLAPPED, timeout_ms: u32) -> io::Result<u32> {
    let wait = unsafe {
        // SAFETY: the event belongs to this live pending operation.
        WaitForSingleObject(overlapped.hEvent, timeout_ms)
    };
    if wait != WAIT_OBJECT_0 {
        let error = if wait == WAIT_TIMEOUT {
            io::Error::new(io::ErrorKind::TimedOut, "pipe I/O operation timed out")
        } else {
            io::Error::last_os_error()
        };
        unsafe {
            // SAFETY: cancel only this operation; cancellation does NOT itself
            // mean completion. Keep all storage alive until the event signals.
            CancelIoEx(raw_handle(file), overlapped);
            if WaitForSingleObject(overlapped.hEvent, CANCELLATION_DRAIN_MS) != WAIT_OBJECT_0 {
                // A kernel/driver failure must not produce use-after-free or an
                // unbounded wait. Fail the broker process, not the host process.
                std::process::abort();
            }
            let mut discarded = 0;
            GetOverlappedResult(raw_handle(file), overlapped, &mut discarded, 0);
        }
        // Even if completion won the cancellation race, the expired request is
        // ambiguous and must close; never retry its bytes or engine command.
        return Err(error);
    }
    let mut transferred = 0;
    let completed = unsafe {
        // SAFETY: the signaled event proves the pending operation completed.
        GetOverlappedResult(raw_handle(file), overlapped, &mut transferred, 0)
    };
    if completed == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(transferred)
    }
}

fn audit_dacl(server: &File, expected_sid: &SidBytes) -> io::Result<bool> {
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    let status = unsafe {
        // SAFETY: output pointers are writable and the pipe handle is valid.
        GetSecurityInfo(
            raw_handle(server),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let descriptor = KernelSecurityDescriptor(descriptor);

    let mut control = 0_u16;
    let mut revision = 0_u32;
    let read_control = unsafe {
        // SAFETY: descriptor is owned and valid for the duration of the query.
        GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision)
    };
    if read_control == 0 {
        return Err(io::Error::last_os_error());
    }
    if control & SE_DACL_PROTECTED == 0 || dacl.is_null() {
        return Ok(false);
    }

    let mut information = ACL_SIZE_INFORMATION::default();
    let read_acl = unsafe {
        // SAFETY: dacl belongs to descriptor and information is a sized output.
        GetAclInformation(
            dacl,
            (&raw mut information).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    };
    if read_acl == 0 {
        return Err(io::Error::last_os_error());
    }
    if information.AceCount != 1 {
        return Ok(false);
    }

    let mut raw_ace = null_mut();
    let read_ace = unsafe {
        // SAFETY: the ACL reports one ACE, so index zero is valid.
        GetAce(dacl, 0, &mut raw_ace)
    };
    if read_ace == 0 {
        return Err(io::Error::last_os_error());
    }
    let ace = unsafe {
        // SAFETY: SDDL created an ACCESS_ALLOWED_ACE and GetAce returned its address.
        &*raw_ace.cast::<ACCESS_ALLOWED_ACE>()
    };
    if u32::from(ace.Header.AceType) != ACCESS_ALLOWED_ACE_TYPE || ace.Mask != CLIENT_PIPE_ACCESS {
        return Ok(false);
    }
    let ace_sid = (&raw const ace.SidStart).cast_mut().cast();
    Ok(unsafe {
        // SAFETY: both pointers identify validated SIDs held by live allocations.
        EqualSid(ace_sid, expected_sid.as_ptr())
    } != 0)
}

#[cfg(test)]
fn peek_pipe(server: &File, header: &mut [u8; mo_ipc::HEADER_LEN]) -> io::Result<(usize, usize)> {
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    let mut bytes_read = 0_u32;
    let mut bytes_available = 0_u32;
    let result = unsafe {
        // SAFETY: server is a connected pipe; header and counters are valid outputs.
        PeekNamedPipe(
            raw_handle(server),
            header.as_mut_ptr().cast(),
            u32::try_from(header.len()).expect("header size fits u32"),
            &mut bytes_read,
            &mut bytes_available,
            null_mut(),
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((bytes_read as usize, bytes_available as usize))
}

fn authenticate_client_logon_sid(server: &File, expected: &SidBytes) -> io::Result<()> {
    let impersonated = unsafe {
        // SAFETY: a first message was read from this connected server pipe.
        ImpersonateNamedPipeClient(raw_handle(server))
    };
    if impersonated == 0 {
        return Err(io::Error::last_os_error());
    }
    let guard = ImpersonationGuard;
    let actual = thread_logon_sid();
    let reverted = guard.revert();
    if reverted.is_err() {
        // Continuing under an untrusted client identity is unsafe.
        std::process::abort();
    }
    let actual = actual?;
    if &actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "named-pipe client belongs to a different logon session",
        ));
    }
    Ok(())
}

struct ImpersonationGuard;

impl ImpersonationGuard {
    fn revert(self) -> io::Result<()> {
        let result = unsafe {
            // SAFETY: the current thread is impersonating the pipe client.
            RevertToSelf()
        };
        std::mem::forget(self);
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for ImpersonationGuard {
    fn drop(&mut self) {
        let result = unsafe {
            // SAFETY: best-effort rollback during unwinding/early return.
            RevertToSelf()
        };
        if result == 0 {
            std::process::abort();
        }
    }
}

fn process_logon_sid() -> io::Result<SidBytes> {
    let mut token = INVALID_HANDLE_VALUE;
    let opened = unsafe {
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle and token is writable.
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
    };
    if opened == 0 {
        return Err(io::Error::last_os_error());
    }
    token_logon_sid(&OwnedHandle(token))
}

fn thread_logon_sid() -> io::Result<SidBytes> {
    let mut token = INVALID_HANDLE_VALUE;
    let opened = unsafe {
        // SAFETY: called while impersonating; token is a writable out-parameter.
        OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token)
    };
    if opened == 0 {
        return Err(io::Error::last_os_error());
    }
    token_logon_sid(&OwnedHandle(token))
}

fn token_logon_sid(token: &OwnedHandle) -> io::Result<SidBytes> {
    let mut required = 0_u32;
    let first = unsafe {
        // SAFETY: the first call intentionally probes the required byte count.
        GetTokenInformation(token.0, TokenGroups, null_mut(), 0, &mut required)
    };
    if first != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0_u8; required as usize];
    let loaded = unsafe {
        // SAFETY: buffer has the exact size returned by the probe call.
        GetTokenInformation(
            token.0,
            TokenGroups,
            buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    };
    if loaded == 0 {
        return Err(io::Error::last_os_error());
    }

    let groups = unsafe {
        // SAFETY: GetTokenInformation initialized a TOKEN_GROUPS record in buffer.
        &*(buffer.as_ptr().cast::<TOKEN_GROUPS>())
    };
    let first_group = groups.Groups.as_ptr();
    for index in 0..groups.GroupCount as usize {
        let group = unsafe {
            // SAFETY: TOKEN_GROUPS contains GroupCount contiguous SID_AND_ATTRIBUTES.
            &*first_group.add(index)
        };
        let logon_id_mask = SE_GROUP_LOGON_ID as u32;
        if group.Attributes & logon_id_mask == logon_id_mask {
            return copy_sid(group.Sid);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "access token does not contain a logon SID",
    ))
}

fn copy_sid(sid: *mut c_void) -> io::Result<SidBytes> {
    use windows_sys::Win32::Security::{GetLengthSid, IsValidSid};

    if unsafe { IsValidSid(sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid token SID",
        ));
    }
    let length = unsafe { GetLengthSid(sid) } as usize;
    let bytes = unsafe {
        // SAFETY: IsValidSid succeeded and GetLengthSid returned its byte length.
        std::slice::from_raw_parts(sid.cast::<u8>(), length)
    };
    Ok(SidBytes(bytes.to_vec()))
}

fn sid_to_string(sid: &SidBytes) -> io::Result<String> {
    let mut pointer = null_mut();
    let converted = unsafe {
        // SAFETY: SidBytes contains a validated, self-contained SID.
        ConvertSidToStringSidW(sid.as_ptr(), &mut pointer)
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = LocalWideString(pointer);
    let length = unsafe {
        // SAFETY: conversion returned a NUL-terminated LocalAlloc string.
        (0..).find(|&index| *pointer.add(index) == 0).unwrap_or(0)
    };
    String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, length) })
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "SID string is not UTF-16"))
        .inspect(|_| drop(owned))
}

struct LocalWideString(*mut u16);

impl Drop for LocalWideString {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: ConvertSidToStringSidW returns LocalAlloc memory.
            LocalFree(self.0.cast());
        }
    }
}

fn file_from_handle(handle: HANDLE) -> io::Result<File> {
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe {
        // SAFETY: successful Win32 creation returned a uniquely owned HANDLE.
        File::from_raw_handle(handle as RawHandle)
    })
}

fn raw_handle(file: &File) -> HANDLE {
    file.as_raw_handle() as HANDLE
}

fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use std::thread;

    use mo_ipc::{CURRENT_VERSION, Frame, MessageKind, write_frame};

    use super::*;

    fn test_address(name: &str) -> PipeAddress {
        PipeAddress::new(&format!("test-{name}-{}", std::process::id())).unwrap()
    }

    #[test]
    fn pool_slots_keep_the_protected_dacl_and_remote_rejection() {
        let base = test_address("pool-acl");
        assert_eq!(pool_slot_address(&base, 0).unwrap(), base);
        assert!(pool_slot_address(&base, MAX_PIPE_SLOTS).is_err());
        assert!(PipePool::bind(base.clone(), 0).is_err());
        assert!(PipePool::bind(base.clone(), MAX_PIPE_SLOTS + 1).is_err());
        let pool = PipePool::bind(base, MAX_PIPE_SLOTS).unwrap();
        let security = SecurityDescriptor::for_logon_sid(&process_logon_sid().unwrap()).unwrap();
        for listener in pool.listeners() {
            assert!(listener.has_expected_dacl().unwrap());
            assert!(listener.rejects_remote_clients().unwrap());
            assert_eq!(
                create_server(listener.address(), &security)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }

    #[test]
    fn a_conflicting_secondary_slot_rolls_back_the_entire_pool_bind() {
        let base = test_address("pool-conflict");
        let occupied = PipeListener::bind(pool_slot_address(&base, 1).unwrap()).unwrap();
        assert_eq!(
            PipePool::bind(base.clone(), 2).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        // The unsuccessfully created primary was closed; the preexisting
        // secondary remains owned by its original listener.
        let primary = PipeListener::bind(base).unwrap();
        assert!(primary.has_expected_dacl().unwrap());
        assert!(occupied.has_expected_dacl().unwrap());
    }

    #[test]
    fn reusable_listener_retains_ownership_and_rejects_a_second_live_stream() {
        let address = test_address("retained-listener");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (done, wait) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (stream, _) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            assert_eq!(
                listener
                    .accept_reusable_first_frame(Duration::ZERO)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            assert_eq!(
                listener
                    .accept_next_first_frame(Duration::ZERO)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            drop(stream);
            assert!(listener.has_expected_dacl().unwrap());
            done.send(()).unwrap();
            listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
        });
        let first = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap();
        let mut client = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        write_frame(&mut client, &first).unwrap();
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            PipeListener::bind(address.clone()).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        drop(client);
        let mut next = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        write_frame(&mut next, &first).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn reply_flush_does_not_wait_for_peer_consumption() {
        let address = test_address("nonblocking-flush");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (done, received) = std::sync::mpsc::sync_channel(1);
        let (release, wait) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut stream, request) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            stream
                .write_frame_with_timeout(&request, Duration::from_millis(100))
                .unwrap();
            stream.flush().unwrap();
            done.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        let mut client = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        let request = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap();
        write_frame(&mut client, &request).unwrap();
        // Keep the client connected and deliberately do not read until flush
        // has returned. FlushFileBuffers would deadlock this handshake.
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(read_frame(&mut client).unwrap(), request);
        release.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn unread_replies_timeout_and_retained_slot_accepts_a_fresh_client() {
        let address = test_address("unread-reply");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (done, received) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            let reply = Frame::new(
                CURRENT_VERSION,
                MessageKind::Ping,
                0,
                0,
                0,
                1,
                vec![b'x'; mo_ipc::MAX_PAYLOAD_LEN],
            )
            .unwrap();
            let started = Instant::now();
            let mut timed_out = false;
            for _ in 0..16 {
                match stream.write_frame_with_timeout(&reply, Duration::from_millis(30)) {
                    Ok(()) => {}
                    Err(error) => {
                        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
                        timed_out = true;
                        break;
                    }
                }
            }
            assert!(
                timed_out,
                "the test must actually fill the kernel pipe buffer"
            );
            assert!(started.elapsed() < Duration::from_secs(1));
            drop(stream);
            assert!(listener.has_expected_dacl().unwrap());
            done.send(()).unwrap();
            let (_, fresh) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            assert_eq!(fresh.header.request_id, 2);
            assert!(fresh.payload.is_empty());
        });
        let mut unread = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        let hello = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap();
        write_frame(&mut unread, &hello).unwrap();
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(unread);
        let mut fresh = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        let hello = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 2, vec![]).unwrap();
        write_frame(&mut fresh, &hello).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn pending_read_cancellation_drains_before_handle_reuse() {
        let address = test_address("cancel-read");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (done, received) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (stream, _) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            let mut byte = [0];
            let error = DeadlineIo::new(&stream.file, Duration::from_millis(20))
                .read(&mut byte)
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            drop(stream);
            done.send(()).unwrap();
            let (_, frame) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            assert_eq!(frame.header.request_id, 2);
        });
        let mut client = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        write_frame(
            &mut client,
            &Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap(),
        )
        .unwrap();
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(client);
        let mut fresh = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        write_frame(
            &mut fresh,
            &Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 2, vec![]).unwrap(),
        )
        .unwrap();
        server.join().unwrap();
    }

    #[test]
    fn activity_frame_deadline_is_not_renewed_by_payload_fragments() {
        let address = test_address("drip-payload");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (ready, received) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            ready.send(()).unwrap();
            let started = Instant::now();
            let error = stream
                .read_frame_after_activity(Duration::from_millis(40))
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            assert!(started.elapsed() < Duration::from_secs(1));
        });
        let mut client = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        let hello = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap();
        write_frame(&mut client, &hello).unwrap();
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        let next = Frame::new(
            CURRENT_VERSION,
            MessageKind::Ping,
            0,
            0,
            0,
            2,
            vec![b'x'; 24],
        )
        .unwrap();
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &next).unwrap();
        client.write_all(&bytes[..mo_ipc::HEADER_LEN + 8]).unwrap();
        thread::sleep(Duration::from_millis(20));
        let _ = client.write_all(&bytes[mo_ipc::HEADER_LEN + 8..mo_ipc::HEADER_LEN + 16]);
        thread::sleep(Duration::from_millis(60));
        let _ = client.write_all(&bytes[mo_ipc::HEADER_LEN + 16..]);
        server.join().unwrap();
    }

    #[test]
    fn idle_wait_does_not_consume_the_next_frame_assembly_budget() {
        let address = test_address("idle-activity");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (ready, received) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            ready.send(()).unwrap();
            let next = stream
                .read_frame_after_activity(Duration::from_millis(20))
                .unwrap();
            assert_eq!(next.header.request_id, 2);
        });
        let mut client = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        write_frame(
            &mut client,
            &Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap(),
        )
        .unwrap();
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        thread::sleep(Duration::from_millis(80));
        write_frame(
            &mut client,
            &Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 2, vec![]).unwrap(),
        )
        .unwrap();
        server.join().unwrap();
    }

    #[test]
    fn expired_frame_budget_does_not_write_any_bytes() {
        let address = test_address("expired-write");
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let (done, received) = std::sync::mpsc::sync_channel(1);
        let (release, wait) = std::sync::mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut stream, request) = listener
                .accept_reusable_first_frame(Duration::from_secs(2))
                .unwrap();
            assert_eq!(
                stream
                    .write_frame_with_timeout(&request, Duration::ZERO)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::TimedOut
            );
            done.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
            // A zero budget never submitted I/O. This explicit test can still
            // send a complete reply; production drops any ambiguous failure.
            stream
                .write_frame_with_timeout(&request, Duration::from_secs(1))
                .unwrap();
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        let mut client = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        let request = Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, vec![]).unwrap();
        write_frame(&mut client, &request).unwrap();
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut header = [0; mo_ipc::HEADER_LEN];
        assert_eq!(peek_pipe(&client, &mut header).unwrap().1, 0);
        release.send(()).unwrap();
        assert_eq!(read_frame(&mut client).unwrap(), request);
        release.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn address_rejects_namespace_escape() {
        assert!(PipeAddress::new("").is_err());
        assert!(PipeAddress::new(r"..\other").is_err());
        assert!(PipeAddress::new("slash/name").is_err());
    }

    #[test]
    fn listener_rejects_remote_clients() {
        let listener = PipeListener::bind(test_address("remote-flag")).unwrap();
        assert!(listener.rejects_remote_clients().unwrap());
        assert!(listener.has_expected_dacl().unwrap());
    }

    #[test]
    fn protected_dacl_denies_a_second_server_instance() {
        let address = test_address("second-instance-denied");
        let listener = PipeListener::bind(address.clone()).unwrap();
        let logon_sid = process_logon_sid().unwrap();
        let security = SecurityDescriptor::for_logon_sid(&logon_sid).unwrap();
        let error = create_server(&address, &security).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        drop(listener);
    }

    #[test]
    fn same_logon_client_is_authenticated_after_bounded_frame() {
        let listener = PipeListener::bind(test_address("roundtrip")).unwrap();
        let address = listener.address().clone();
        let client = thread::spawn(move || {
            let mut stream = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
            let frame =
                Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, Vec::new()).unwrap();
            write_frame(&mut stream, &frame).unwrap();
        });

        let (_server, frame) = listener.accept_first_frame(Duration::from_secs(2)).unwrap();
        assert_eq!(frame.header.kind, MessageKind::Ping);
        assert_eq!(frame.header.request_id, 1);
        client.join().unwrap();
    }

    #[test]
    fn silent_client_hits_first_frame_deadline() {
        let listener = PipeListener::bind(test_address("first-frame-timeout")).unwrap();
        let address = listener.address().clone();
        let client = thread::spawn(move || {
            let _stream = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
            thread::sleep(Duration::from_millis(100));
        });

        let error = listener
            .accept_first_frame(Duration::from_millis(20))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        client.join().unwrap();
    }

    #[test]
    fn missing_listener_honors_the_total_connect_deadline() {
        let address = test_address("missing-listener");
        let started = Instant::now();
        let error = PipeClient::connect(&address, Duration::from_millis(20)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn authenticated_partial_frame_hits_assembly_deadline() {
        let listener = PipeListener::bind(test_address("partial-frame-timeout")).unwrap();
        let address = listener.address().clone();
        let client = thread::spawn(move || {
            let mut stream = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
            let frame =
                Frame::new(CURRENT_VERSION, MessageKind::Ping, 0, 0, 0, 1, Vec::new()).unwrap();
            write_frame(&mut stream, &frame).unwrap();
            stream.write_all(b"M").unwrap();
            thread::sleep(Duration::from_millis(100));
        });

        let (mut server, _) = listener.accept_first_frame(Duration::from_secs(2)).unwrap();
        let error = server
            .read_frame_after_activity(Duration::from_millis(20))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        client.join().unwrap();
    }
}
