//! A deliberately small mirror of the prefix of librime's public C API used by
//! Mo.
//!
//! The authoritative declaration is `src/rime_api.h` from the librime revision
//! recorded under `native/librime/UPSTREAM.toml`.  This crate does not model
//! librime C++ objects and does not link a native library unless one of the
//! `link-*` features is selected.

#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_char, c_int, c_void};
use std::mem::size_of;

/// librime declares `Bool` as the C type `int`.
pub type RimeBool = c_int;
pub const RIME_FALSE: RimeBool = 0;
pub const RIME_TRUE: RimeBool = 1;

/// librime declares a session handle as `uintptr_t`.
pub type RimeSessionId = usize;
pub const RIME_NO_SESSION: RimeSessionId = 0;

#[inline]
pub const fn to_rime_bool(value: bool) -> RimeBool {
    if value { RIME_TRUE } else { RIME_FALSE }
}

#[inline]
pub const fn from_rime_bool(value: RimeBool) -> bool {
    value != RIME_FALSE
}

/// Implements the `RIME_STRUCT_INIT` size rule.  The C API records the bytes
/// after `data_size`, not `size_of::<T>()` itself.
#[inline]
pub const fn rime_struct_data_size<T>() -> c_int {
    (size_of::<T>() - size_of::<c_int>()) as c_int
}

/// A stricter form of `RIME_STRUCT_HAS_MEMBER`: the complete field, rather than
/// only its first byte, must be covered by the advertised size.
#[inline]
pub const fn advertised_range_available(
    data_size: c_int,
    field_offset: usize,
    field_size: usize,
) -> bool {
    if data_size < 0 {
        return false;
    }
    size_of::<c_int>() + data_size as usize >= field_offset + field_size
}

/// Exact mirror of `RimeTraits` in the pinned public header.
#[repr(C)]
pub struct RimeTraits {
    pub data_size: c_int,
    pub shared_data_dir: *const c_char,
    pub user_data_dir: *const c_char,
    pub distribution_name: *const c_char,
    pub distribution_code_name: *const c_char,
    pub distribution_version: *const c_char,
    pub app_name: *const c_char,
    pub modules: *mut *const c_char,
    pub min_log_level: c_int,
    pub log_dir: *const c_char,
    pub prebuilt_data_dir: *const c_char,
    pub staging_dir: *const c_char,
}

impl Default for RimeTraits {
    fn default() -> Self {
        Self {
            data_size: rime_struct_data_size::<Self>(),
            shared_data_dir: std::ptr::null(),
            user_data_dir: std::ptr::null(),
            distribution_name: std::ptr::null(),
            distribution_code_name: std::ptr::null(),
            distribution_version: std::ptr::null(),
            app_name: std::ptr::null(),
            modules: std::ptr::null_mut(),
            min_log_level: 0,
            log_dir: std::ptr::null(),
            prebuilt_data_dir: std::ptr::null(),
            staging_dir: std::ptr::null(),
        }
    }
}

#[repr(C)]
pub struct RimeComposition {
    pub length: c_int,
    pub cursor_pos: c_int,
    pub sel_start: c_int,
    pub sel_end: c_int,
    pub preedit: *mut c_char,
}

impl Default for RimeComposition {
    fn default() -> Self {
        Self {
            length: 0,
            cursor_pos: 0,
            sel_start: 0,
            sel_end: 0,
            preedit: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct RimeCandidate {
    pub text: *mut c_char,
    pub comment: *mut c_char,
    pub reserved: *mut c_void,
}

impl Default for RimeCandidate {
    fn default() -> Self {
        Self {
            text: std::ptr::null_mut(),
            comment: std::ptr::null_mut(),
            reserved: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct RimeMenu {
    pub page_size: c_int,
    pub page_no: c_int,
    pub is_last_page: RimeBool,
    pub highlighted_candidate_index: c_int,
    pub num_candidates: c_int,
    pub candidates: *mut RimeCandidate,
    pub select_keys: *mut c_char,
}

impl Default for RimeMenu {
    fn default() -> Self {
        Self {
            page_size: 0,
            page_no: 0,
            is_last_page: RIME_FALSE,
            highlighted_candidate_index: 0,
            num_candidates: 0,
            candidates: std::ptr::null_mut(),
            select_keys: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct RimeCommit {
    pub data_size: c_int,
    pub text: *mut c_char,
}

impl Default for RimeCommit {
    fn default() -> Self {
        Self {
            data_size: rime_struct_data_size::<Self>(),
            text: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct RimeContext {
    pub data_size: c_int,
    pub composition: RimeComposition,
    pub menu: RimeMenu,
    pub commit_text_preview: *mut c_char,
    pub select_labels: *mut *mut c_char,
}

impl Default for RimeContext {
    fn default() -> Self {
        Self {
            data_size: rime_struct_data_size::<Self>(),
            composition: RimeComposition::default(),
            menu: RimeMenu::default(),
            commit_text_preview: std::ptr::null_mut(),
            select_labels: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct RimeStatus {
    pub data_size: c_int,
    pub schema_id: *mut c_char,
    pub schema_name: *mut c_char,
    pub is_disabled: RimeBool,
    pub is_composing: RimeBool,
    pub is_ascii_mode: RimeBool,
    pub is_full_shape: RimeBool,
    pub is_simplified: RimeBool,
    pub is_traditional: RimeBool,
    pub is_ascii_punct: RimeBool,
}

impl Default for RimeStatus {
    fn default() -> Self {
        Self {
            data_size: rime_struct_data_size::<Self>(),
            schema_id: std::ptr::null_mut(),
            schema_name: std::ptr::null_mut(),
            is_disabled: RIME_FALSE,
            is_composing: RIME_FALSE,
            is_ascii_mode: RIME_FALSE,
            is_full_shape: RIME_FALSE,
            is_simplified: RIME_FALSE,
            is_traditional: RIME_FALSE,
            is_ascii_punct: RIME_FALSE,
        }
    }
}

pub type RimeNotificationHandler = Option<
    unsafe extern "C" fn(
        context_object: *mut c_void,
        session_id: RimeSessionId,
        message_type: *const c_char,
        message_value: *const c_char,
    ),
>;

pub type SetupFn = Option<unsafe extern "C" fn(*mut RimeTraits)>;
pub type SetNotificationHandlerFn =
    Option<unsafe extern "C" fn(RimeNotificationHandler, *mut c_void)>;
pub type InitializeFn = Option<unsafe extern "C" fn(*mut RimeTraits)>;
pub type FinalizeFn = Option<unsafe extern "C" fn()>;
pub type StartMaintenanceFn = Option<unsafe extern "C" fn(RimeBool) -> RimeBool>;
pub type IsMaintenanceModeFn = Option<unsafe extern "C" fn() -> RimeBool>;
pub type JoinMaintenanceThreadFn = Option<unsafe extern "C" fn()>;
pub type DeployerInitializeFn = Option<unsafe extern "C" fn(*mut RimeTraits)>;
pub type PrebuildFn = Option<unsafe extern "C" fn() -> RimeBool>;
pub type DeployFn = Option<unsafe extern "C" fn() -> RimeBool>;
pub type DeploySchemaFn = Option<unsafe extern "C" fn(*const c_char) -> RimeBool>;
pub type DeployConfigFileFn =
    Option<unsafe extern "C" fn(*const c_char, *const c_char) -> RimeBool>;
pub type SyncUserDataFn = Option<unsafe extern "C" fn() -> RimeBool>;
pub type CreateSessionFn = Option<unsafe extern "C" fn() -> RimeSessionId>;
pub type FindSessionFn = Option<unsafe extern "C" fn(RimeSessionId) -> RimeBool>;
pub type DestroySessionFn = Option<unsafe extern "C" fn(RimeSessionId) -> RimeBool>;
pub type CleanupStaleSessionsFn = Option<unsafe extern "C" fn()>;
pub type CleanupAllSessionsFn = Option<unsafe extern "C" fn()>;
pub type ProcessKeyFn = Option<unsafe extern "C" fn(RimeSessionId, c_int, c_int) -> RimeBool>;
pub type CommitCompositionFn = Option<unsafe extern "C" fn(RimeSessionId) -> RimeBool>;
pub type ClearCompositionFn = Option<unsafe extern "C" fn(RimeSessionId)>;
pub type GetCommitFn = Option<unsafe extern "C" fn(RimeSessionId, *mut RimeCommit) -> RimeBool>;
pub type FreeCommitFn = Option<unsafe extern "C" fn(*mut RimeCommit) -> RimeBool>;
pub type GetContextFn = Option<unsafe extern "C" fn(RimeSessionId, *mut RimeContext) -> RimeBool>;
pub type FreeContextFn = Option<unsafe extern "C" fn(*mut RimeContext) -> RimeBool>;
pub type GetStatusFn = Option<unsafe extern "C" fn(RimeSessionId, *mut RimeStatus) -> RimeBool>;
pub type FreeStatusFn = Option<unsafe extern "C" fn(*mut RimeStatus) -> RimeBool>;

/// Prefix of `RimeApi` through `free_status`, preserving every intervening
/// field in the official order.  Mo validates `data_size` before reading this
/// prefix.  Later API members will be added only when the safe layer needs them.
#[repr(C)]
pub struct RimeApi {
    pub data_size: c_int,
    pub setup: SetupFn,
    pub set_notification_handler: SetNotificationHandlerFn,
    pub initialize: InitializeFn,
    pub finalize: FinalizeFn,
    pub start_maintenance: StartMaintenanceFn,
    pub is_maintenance_mode: IsMaintenanceModeFn,
    pub join_maintenance_thread: JoinMaintenanceThreadFn,
    pub deployer_initialize: DeployerInitializeFn,
    pub prebuild: PrebuildFn,
    pub deploy: DeployFn,
    pub deploy_schema: DeploySchemaFn,
    pub deploy_config_file: DeployConfigFileFn,
    pub sync_user_data: SyncUserDataFn,
    pub create_session: CreateSessionFn,
    pub find_session: FindSessionFn,
    pub destroy_session: DestroySessionFn,
    pub cleanup_stale_sessions: CleanupStaleSessionsFn,
    pub cleanup_all_sessions: CleanupAllSessionsFn,
    pub process_key: ProcessKeyFn,
    pub commit_composition: CommitCompositionFn,
    pub clear_composition: ClearCompositionFn,
    pub get_commit: GetCommitFn,
    pub free_commit: FreeCommitFn,
    pub get_context: GetContextFn,
    pub free_context: FreeContextFn,
    pub get_status: GetStatusFn,
    pub free_status: FreeStatusFn,
}

impl Default for RimeApi {
    fn default() -> Self {
        Self {
            data_size: rime_struct_data_size::<Self>(),
            setup: None,
            set_notification_handler: None,
            initialize: None,
            finalize: None,
            start_maintenance: None,
            is_maintenance_mode: None,
            join_maintenance_thread: None,
            deployer_initialize: None,
            prebuild: None,
            deploy: None,
            deploy_schema: None,
            deploy_config_file: None,
            sync_user_data: None,
            create_session: None,
            find_session: None,
            destroy_session: None,
            cleanup_stale_sessions: None,
            cleanup_all_sessions: None,
            process_key: None,
            commit_composition: None,
            clear_composition: None,
            get_commit: None,
            free_commit: None,
            get_context: None,
            free_context: None,
            get_status: None,
            free_status: None,
        }
    }
}

/// Minimum advertised API payload needed to read the prefix above.
pub const RIME_API_REQUIRED_DATA_SIZE: c_int = rime_struct_data_size::<RimeApi>();

#[cfg(any(feature = "link-dynamic", feature = "link-static"))]
unsafe extern "C" {
    pub fn rime_get_api() -> *mut RimeApi;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn c_integer_and_handle_widths_are_not_rust_bool_or_fixed_u64() {
        assert_eq!(size_of::<RimeBool>(), size_of::<c_int>());
        assert_eq!(size_of::<RimeSessionId>(), size_of::<usize>());
        assert_eq!(to_rime_bool(false), 0);
        assert_eq!(to_rime_bool(true), 1);
        assert!(from_rime_bool(-7));
    }

    #[test]
    fn versioned_structs_exclude_the_data_size_field() {
        assert_eq!(
            RimeTraits::default().data_size as usize,
            size_of::<RimeTraits>() - size_of::<c_int>()
        );
        assert_eq!(
            RimeCommit::default().data_size as usize,
            size_of::<RimeCommit>() - size_of::<c_int>()
        );
        assert_eq!(
            RimeContext::default().data_size as usize,
            size_of::<RimeContext>() - size_of::<c_int>()
        );
        assert_eq!(
            RimeStatus::default().data_size as usize,
            size_of::<RimeStatus>() - size_of::<c_int>()
        );
    }

    #[test]
    fn complete_api_prefix_is_required_before_access() {
        let end = offset_of!(RimeApi, free_status) + size_of::<FreeStatusFn>();
        assert_eq!(
            RIME_API_REQUIRED_DATA_SIZE as usize + size_of::<c_int>(),
            size_of::<RimeApi>()
        );
        assert!(advertised_range_available(
            RIME_API_REQUIRED_DATA_SIZE,
            offset_of!(RimeApi, free_status),
            size_of::<FreeStatusFn>()
        ));
        assert!(!advertised_range_available(
            (end - size_of::<c_int>() - 1) as c_int,
            offset_of!(RimeApi, free_status),
            size_of::<FreeStatusFn>()
        ));
        assert!(!advertised_range_available(
            -1,
            offset_of!(RimeApi, setup),
            size_of::<SetupFn>()
        ));
        assert!(align_of::<RimeApi>() >= align_of::<usize>());
    }
}
