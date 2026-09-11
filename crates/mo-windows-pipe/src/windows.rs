use std::ffi::{OsStr, c_void};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use mo_ipc::{Frame, read_frame};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_PIPE_CONNECTED, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, LocalFree,
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
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeInfo,
    ImpersonateNamedPipeClient, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PeekNamedPipe, WaitNamedPipeW,
};
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, SE_GROUP_LOGON_ID};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken,
};

const PIPE_PREFIX: &str = r"\\.\pipe\LOCAL\Mo.Input.";
const PIPE_BUFFER_BYTES: u32 = (mo_ipc::MAX_FRAME_LEN as u32) * 2;
const SDDL_REVISION_1: u32 = 1;
const CLIENT_PIPE_ACCESS: u32 = 0x0012_019b;
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;

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
        })
    }

    pub fn address(&self) -> &PipeAddress {
        &self.address
    }

    /// Accepts one client, reads its first bounded frame, then authenticates it.
    ///
    /// Consuming the listener makes the initial single-instance constraint
    /// explicit. A later overlapped listener pool can extend concurrency without
    /// weakening the DACL or peer check.
    pub fn accept_first_frame(
        mut self,
        first_frame_timeout: Duration,
    ) -> io::Result<(AuthenticatedPipe, Frame)> {
        let mut server = self
            .server
            .take()
            .expect("listener owns its server instance");
        connect_server(&server)?;
        wait_for_complete_frame(&server, first_frame_timeout)?;
        let frame = read_frame(&mut server)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        authenticate_client_logon_sid(&server, &self.logon_sid)?;
        Ok((AuthenticatedPipe { file: server }, frame))
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
}

impl Read for AuthenticatedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

impl Write for AuthenticatedPipe {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.file.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
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
        let timeout_ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        let address_wide = address.wide();
        let waited = unsafe {
            // SAFETY: the address is NUL-terminated and remains alive for the call.
            WaitNamedPipeW(address_wide.as_ptr(), timeout_ms)
        };
        if waited == 0 {
            return Err(io::Error::last_os_error());
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
        file_from_handle(handle)
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
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
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
    let connected = unsafe {
        // SAFETY: server owns a synchronous named-pipe server handle.
        ConnectNamedPipe(raw_handle(server), null_mut())
    };
    if connected != 0 {
        return Ok(());
    }
    let error = unsafe { GetLastError() };
    if error == ERROR_PIPE_CONNECTED {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error as i32))
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

fn wait_for_complete_frame(server: &File, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    let mut header = [0_u8; mo_ipc::HEADER_LEN];

    loop {
        let (peeked, available) = peek_pipe(server, &mut header)?;
        if peeked >= mo_ipc::HEADER_LEN {
            let declared = u32::from_le_bytes(
                header[16..20]
                    .try_into()
                    .expect("payload length has a fixed header position"),
            ) as usize;
            if declared > mo_ipc::MAX_PAYLOAD_LEN {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "declared payload length {declared} exceeds maximum {}",
                        mo_ipc::MAX_PAYLOAD_LEN
                    ),
                ));
            }
            let frame_len = mo_ipc::HEADER_LEN + declared;
            if available >= frame_len {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for the first complete IPC frame",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn peek_pipe(server: &File, header: &mut [u8; mo_ipc::HEADER_LEN]) -> io::Result<(usize, usize)> {
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
}
