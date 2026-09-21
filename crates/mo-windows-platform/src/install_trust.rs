use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT,
};
#[cfg(test)]
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation,
    DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetAclInformation, OWNER_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID,
};
#[cfg(test)]
use windows_sys::Win32::Security::{GetSecurityDescriptorDacl, GetSecurityDescriptorOwner};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFileInformationByHandle, OPEN_EXISTING,
};
use windows_sys::Win32::System::SystemServices::{
    ACCESS_ALLOWED_ACE_TYPE, ACCESS_ALLOWED_CALLBACK_ACE_TYPE,
    ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE, ACCESS_ALLOWED_COMPOUND_ACE_TYPE,
    ACCESS_ALLOWED_OBJECT_ACE_TYPE,
};

const READ_CONTROL: u32 = 0x0002_0000;
const DANGEROUS_FILE_RIGHTS: u32 = 0x500d_0156;
const TRUSTED_INSTALLER_SID: &str =
    "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
const SYSTEM_SID: &str = "S-1-5-18";
const ADMINISTRATORS_SID: &str = "S-1-5-32-544";
const CREATOR_OWNER_SID: &str = "S-1-3-0";

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: each wrapper owns one allocation returned by a Win32
            // conversion/security API whose contract requires LocalFree.
            unsafe { LocalFree(self.0) };
        }
    }
}

struct LocalSid {
    allocation: LocalAllocation,
}

impl LocalSid {
    fn parse(value: &str) -> io::Result<Self> {
        let wide = wide(value.as_ref());
        let mut sid = null_mut();
        let converted = unsafe {
            // SAFETY: wide is terminated and sid is a writable output pointer.
            ConvertStringSidToSidW(wide.as_ptr(), &mut sid)
        };
        if converted == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            allocation: LocalAllocation(sid),
        })
    }

    fn as_ptr(&self) -> PSID {
        self.allocation.0
    }
}

struct TrustedSids {
    system: LocalSid,
    administrators: LocalSid,
    trusted_installer: LocalSid,
    creator_owner: LocalSid,
}

impl TrustedSids {
    fn new() -> io::Result<Self> {
        Ok(Self {
            system: LocalSid::parse(SYSTEM_SID)?,
            administrators: LocalSid::parse(ADMINISTRATORS_SID)?,
            trusted_installer: LocalSid::parse(TRUSTED_INSTALLER_SID)?,
            creator_owner: LocalSid::parse(CREATOR_OWNER_SID)?,
        })
    }

    fn trusted_owner(&self, sid: PSID) -> bool {
        self.equal(sid, &self.system)
            || self.equal(sid, &self.administrators)
            || self.equal(sid, &self.trusted_installer)
    }

    fn trusted_writer(&self, sid: PSID) -> bool {
        self.trusted_owner(sid) || self.equal(sid, &self.creator_owner)
    }

    fn equal(&self, left: PSID, right: &LocalSid) -> bool {
        !left.is_null()
            && unsafe {
                // SAFETY: both values are valid SIDs owned by a live security
                // descriptor or LocalSid allocation.
                EqualSid(left, right.as_ptr())
            } != 0
    }
}

#[repr(C)]
struct AllowedAcePrefix {
    header: ACE_HEADER,
    mask: u32,
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

fn open_for_audit(path: &Path) -> io::Result<File> {
    let wide = wide(path.as_os_str());
    let handle = unsafe {
        // SAFETY: the path is terminated; the returned handle is exclusively
        // transferred into File below. OPEN_REPARSE_POINT prevents traversal.
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe {
        // SAFETY: handle is valid and ownership is transferred exactly once.
        File::from_raw_handle(handle)
    })
}

fn raw_handle(file: &File) -> HANDLE {
    file.as_raw_handle()
}

fn audit_owner_and_dacl(owner: PSID, dacl: *mut ACL, label: &Path) -> io::Result<()> {
    let trusted = TrustedSids::new()?;
    if owner.is_null() || !trusted.trusted_owner(owner) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("installed path owner is not trusted: {}", label.display()),
        ));
    }
    if dacl.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "installed path has a missing/null DACL: {}",
                label.display()
            ),
        ));
    }
    let mut information = ACL_SIZE_INFORMATION::default();
    let read = unsafe {
        // SAFETY: dacl comes from a live security descriptor; information is a
        // correctly sized writable output.
        GetAclInformation(
            dacl,
            (&raw mut information).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    for index in 0..information.AceCount {
        let mut raw_ace = null_mut();
        let read = unsafe {
            // SAFETY: index is bounded by the ACE count returned for this ACL.
            GetAce(dacl, index, &mut raw_ace)
        };
        if read == 0 {
            return Err(io::Error::last_os_error());
        }
        let prefix = unsafe {
            // SAFETY: every access-control ACE begins with ACE_HEADER and Mask.
            &*raw_ace.cast::<AllowedAcePrefix>()
        };
        let ace_type = u32::from(prefix.header.AceType);
        if !matches!(
            ace_type,
            ACCESS_ALLOWED_ACE_TYPE
                | ACCESS_ALLOWED_COMPOUND_ACE_TYPE
                | ACCESS_ALLOWED_OBJECT_ACE_TYPE
                | ACCESS_ALLOWED_CALLBACK_ACE_TYPE
                | ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE
        ) || prefix.mask & DANGEROUS_FILE_RIGHTS == 0
        {
            continue;
        }
        if ace_type != ACCESS_ALLOWED_ACE_TYPE {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "installed path contains unsupported write-like allow ACE: {}",
                    label.display()
                ),
            ));
        }
        let ace = unsafe {
            // SAFETY: the type above is ACCESS_ALLOWED_ACE_TYPE.
            &*raw_ace.cast::<ACCESS_ALLOWED_ACE>()
        };
        let sid = (&raw const ace.SidStart).cast_mut().cast();
        if !trusted.trusted_writer(sid) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "installed path grants write-like access to untrusted SID {}: {}",
                    sid_text(sid).unwrap_or_else(|_| "<invalid>".to_owned()),
                    label.display()
                ),
            ));
        }
    }
    Ok(())
}

fn sid_text(sid: PSID) -> io::Result<String> {
    let mut output = null_mut();
    let converted = unsafe {
        // SAFETY: sid belongs to a live descriptor and output is writable.
        ConvertSidToStringSidW(sid, &mut output)
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    let allocation = LocalAllocation(output.cast());
    let mut length = 0;
    while unsafe {
        // SAFETY: ConvertSidToStringSidW returned a terminated LocalAlloc string.
        *output.add(length)
    } != 0
    {
        length += 1;
    }
    let text = String::from_utf16_lossy(unsafe {
        // SAFETY: length was found before the allocation is released.
        std::slice::from_raw_parts(output, length)
    });
    drop(allocation);
    Ok(text)
}

fn audit_security(file: &File, label: &Path) -> io::Result<()> {
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let status = unsafe {
        // SAFETY: file is a live filesystem handle and all requested outputs
        // are writable for the duration of this call.
        GetSecurityInfo(
            raw_handle(file),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _descriptor = LocalAllocation(descriptor);
    audit_owner_and_dacl(owner, dacl, label)
}

fn audit_path(path: &Path, expected_directory: Option<bool>) -> io::Result<bool> {
    let file = open_for_audit(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to open installed path {}: {error}", path.display()),
        )
    })?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let read = unsafe {
        // SAFETY: file is live and information is a correctly sized output.
        GetFileInformationByHandle(raw_handle(&file), &mut information)
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("installed path is a reparse point: {}", path.display()),
        ));
    }
    let is_directory = information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    if expected_directory.is_some_and(|expected| expected != is_directory) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("installed path type mismatch: {}", path.display()),
        ));
    }
    if !is_directory && information.nNumberOfLinks != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "installed file has {} hard links: {}",
                information.nNumberOfLinks,
                path.display()
            ),
        ));
    }
    audit_security(&file, path)?;
    Ok(is_directory)
}

pub(super) fn validate_installation_tree(
    program_files_x64: &Path,
    install_root: &Path,
) -> io::Result<()> {
    if !program_files_x64.is_absolute()
        || install_root != program_files_x64.join("Mo")
        || !install_root.is_absolute()
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "installation trust root must be the fixed Program Files/Mo path",
        ));
    }
    audit_path(program_files_x64, Some(true))?;
    audit_path(install_root, Some(true))?;
    let mut pending = vec![install_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let path = entry?.path();
            if audit_path(&path, None)? {
                pending.push(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn audit_sddl(sddl: &str) -> io::Result<()> {
    let wide = wide(sddl.as_ref());
    let mut descriptor = null_mut();
    let converted = unsafe {
        // SAFETY: wide is terminated and descriptor is a writable output.
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    let descriptor_owner = LocalAllocation(descriptor);
    let mut owner = null_mut();
    let mut owner_defaulted = 0;
    let mut dacl = null_mut();
    let mut dacl_present = 0;
    let mut dacl_defaulted = 0;
    if unsafe {
        // SAFETY: descriptor_owner is a valid self-relative descriptor and
        // every output is writable.
        GetSecurityDescriptorOwner(descriptor, &mut owner, &mut owner_defaulted)
    } == 0
        || unsafe {
            // SAFETY: same descriptor and valid outputs as above.
            GetSecurityDescriptorDacl(
                descriptor,
                &mut dacl_present,
                &mut dacl,
                &mut dacl_defaulted,
            )
        } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if dacl_present == 0 {
        dacl = null_mut();
    }
    let result = audit_owner_and_dacl(owner, dacl, Path::new("synthetic-sddl"));
    drop(descriptor_owner);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("mo-install-trust-{}-{nonce}", std::process::id()));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn sddl_policy_rejects_untrusted_owner_and_writers() {
        audit_sddl("O:BAG:SYD:PAI(A;;FA;;;SY)(A;;FA;;;BA)(A;;FA;;;CO)(A;;0x1200A9;;;BU)").unwrap();
        for sddl in [
            "O:BUG:SYD:PAI(A;;FA;;;SY)(A;;0x1200A9;;;BU)",
            "O:BAG:SYD:PAI(A;;FA;;;SY)(A;;FA;;;BU)",
            "O:BAG:SYD:PAI(A;;FA;;;SY)(A;;GW;;;AU)",
        ] {
            assert_eq!(
                audit_sddl(sddl).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }

    #[test]
    fn null_dacl_is_rejected_even_for_a_trusted_owner() {
        let owner = LocalSid::parse(ADMINISTRATORS_SID).unwrap();
        let error =
            audit_owner_and_dacl(owner.as_ptr(), null_mut(), Path::new("synthetic-null-dacl"))
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("missing/null DACL"));
    }

    #[test]
    fn program_files_live_acl_matches_the_runtime_policy() {
        let roots = super::super::runtime_roots().unwrap();
        audit_path(&roots.program_files_x64, Some(true)).unwrap();
    }

    #[test]
    fn multiple_hard_links_are_rejected_before_acl_trust() {
        let fixture = Fixture::new();
        let original = fixture.0.join("original.bin");
        let alias = fixture.0.join("alias.bin");
        std::fs::write(&original, b"fixture").unwrap();
        std::fs::hard_link(&original, &alias).unwrap();
        let error = audit_path(&original, Some(false)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("hard links"));
    }

    #[test]
    fn caller_cannot_select_a_foreign_install_root() {
        let roots = super::super::runtime_roots().unwrap();
        let error = validate_installation_tree(
            &roots.program_files_x64,
            &roots.program_files_x64.join("NotMo"),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
