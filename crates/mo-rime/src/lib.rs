//! Safe, single-threaded ownership boundary around the librime C API.
//!
//! Every native snapshot is copied before its matching `free_*` call.  Engine
//! and session handles are deliberately `!Send + !Sync`; the future engine
//! actor is the only intended owner.

#![deny(unsafe_op_in_unsafe_fn)]

use mo_rime_sys as sys;
use std::ffi::{CStr, CString, c_char, c_int};
use std::fmt;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, TryLockError};

mod backend;
#[cfg(windows)]
mod windows_loader;

pub use backend::{RimeBackend, RimeBackendError, RimeBackendSession};
#[cfg(windows)]
pub use windows_loader::RuntimeLibraryError;

const MAX_CANDIDATES_PER_PAGE: usize = 1_024;
const MAX_SELECT_LABELS: usize = 256;

static ENGINE_GATE: Mutex<()> = Mutex::new(());

// Versioned Mo-only C export, not an upstream RimeApi slot.
type PrepareResources = unsafe extern "C" fn(sys::RimeSessionId) -> c_int;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NullApi,
    ApiTooOld {
        advertised: c_int,
        required: c_int,
    },
    MissingFunction(&'static str),
    EngineAlreadyActive,
    EngineGatePoisoned,
    InteriorNul(&'static str),
    SessionCreationFailed,
    NativeReleaseFailed(&'static str),
    NativeCallFailed(&'static str),
    InvalidCount {
        field: &'static str,
        value: c_int,
    },
    NullArray {
        field: &'static str,
        count: usize,
    },
    AllocationFailed(&'static str),
    #[cfg(windows)]
    RuntimeLibrary(RuntimeLibraryError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullApi => formatter.write_str("rime_get_api returned NULL"),
            Self::ApiTooOld {
                advertised,
                required,
            } => write!(
                formatter,
                "librime API table is too old: advertised {advertised} bytes, need {required}"
            ),
            Self::MissingFunction(name) => {
                write!(formatter, "librime API function `{name}` is unavailable")
            }
            Self::EngineAlreadyActive => {
                formatter.write_str("a librime engine is already active in this process")
            }
            Self::EngineGatePoisoned => formatter.write_str("the librime engine gate is poisoned"),
            Self::InteriorNul(field) => write!(formatter, "`{field}` contains a NUL byte"),
            Self::SessionCreationFailed => {
                formatter.write_str("librime failed to create a session")
            }
            Self::NativeReleaseFailed(kind) => {
                write!(formatter, "librime failed to release {kind}")
            }
            Self::NativeCallFailed(call) => write!(formatter, "librime call `{call}` failed"),
            Self::InvalidCount { field, value } => {
                write!(formatter, "invalid native count `{field}`: {value}")
            }
            Self::NullArray { field, count } => {
                write!(
                    formatter,
                    "native array `{field}` is NULL with count {count}"
                )
            }
            Self::AllocationFailed(field) => {
                write!(
                    formatter,
                    "failed to allocate owned snapshot field `{field}`"
                )
            }
            #[cfg(windows)]
            Self::RuntimeLibrary(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(windows)]
impl From<RuntimeLibraryError> for Error {
    fn from(error: RuntimeLibraryError) -> Self {
        Self::RuntimeLibrary(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    pub shared_data_dir: String,
    pub user_data_dir: String,
    pub distribution_name: String,
    pub distribution_code_name: String,
    pub distribution_version: String,
    pub app_name: String,
    pub modules: Vec<String>,
    pub min_log_level: c_int,
    pub log_dir: Option<String>,
    pub prebuilt_data_dir: Option<String>,
    pub staging_dir: Option<String>,
}

impl EngineConfig {
    pub fn new(shared_data_dir: impl Into<String>, user_data_dir: impl Into<String>) -> Self {
        Self {
            shared_data_dir: shared_data_dir.into(),
            user_data_dir: user_data_dir.into(),
            distribution_name: "Mo Input Method".to_owned(),
            distribution_code_name: "Mo".to_owned(),
            distribution_version: env!("CARGO_PKG_VERSION").to_owned(),
            app_name: "rime.mo".to_owned(),
            // Mo's pinned Windows librime distribution includes librime-lua,
            // and the pinned rime-ice schemas require its processors,
            // translators and filters. Callers can still replace this list
            // when testing a deliberately reduced native build.
            modules: vec!["default".to_owned(), "lua".to_owned()],
            min_log_level: 1,
            log_dir: None,
            prebuilt_data_dir: None,
            staging_dir: None,
        }
    }
}

struct TraitStorage {
    raw: sys::RimeTraits,
    _shared_data_dir: CString,
    _user_data_dir: CString,
    _distribution_name: CString,
    _distribution_code_name: CString,
    _distribution_version: CString,
    _app_name: CString,
    _log_dir: Option<CString>,
    _prebuilt_data_dir: Option<CString>,
    _staging_dir: Option<CString>,
    _modules: Vec<CString>,
    _module_pointers: Vec<*const c_char>,
}

impl TraitStorage {
    fn new(config: EngineConfig) -> Result<Box<Self>, Error> {
        let shared_data_dir = c_string("shared_data_dir", config.shared_data_dir)?;
        let user_data_dir = c_string("user_data_dir", config.user_data_dir)?;
        let distribution_name = c_string("distribution_name", config.distribution_name)?;
        let distribution_code_name =
            c_string("distribution_code_name", config.distribution_code_name)?;
        let distribution_version = c_string("distribution_version", config.distribution_version)?;
        let app_name = c_string("app_name", config.app_name)?;
        let log_dir = optional_c_string("log_dir", config.log_dir)?;
        let prebuilt_data_dir = optional_c_string("prebuilt_data_dir", config.prebuilt_data_dir)?;
        let staging_dir = optional_c_string("staging_dir", config.staging_dir)?;

        let modules = config
            .modules
            .into_iter()
            .map(|value| c_string("modules", value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut module_pointers = Vec::new();
        module_pointers
            .try_reserve_exact(modules.len() + 1)
            .map_err(|_| Error::AllocationFailed("modules"))?;
        module_pointers.extend(modules.iter().map(|module| module.as_ptr()));
        module_pointers.push(std::ptr::null());

        let raw = sys::RimeTraits {
            data_size: sys::rime_struct_data_size::<sys::RimeTraits>(),
            shared_data_dir: shared_data_dir.as_ptr(),
            user_data_dir: user_data_dir.as_ptr(),
            distribution_name: distribution_name.as_ptr(),
            distribution_code_name: distribution_code_name.as_ptr(),
            distribution_version: distribution_version.as_ptr(),
            app_name: app_name.as_ptr(),
            modules: module_pointers.as_mut_ptr(),
            min_log_level: config.min_log_level,
            log_dir: optional_pointer(&log_dir),
            prebuilt_data_dir: optional_pointer(&prebuilt_data_dir),
            staging_dir: optional_pointer(&staging_dir),
        };

        Ok(Box::new(Self {
            raw,
            _shared_data_dir: shared_data_dir,
            _user_data_dir: user_data_dir,
            _distribution_name: distribution_name,
            _distribution_code_name: distribution_code_name,
            _distribution_version: distribution_version,
            _app_name: app_name,
            _log_dir: log_dir,
            _prebuilt_data_dir: prebuilt_data_dir,
            _staging_dir: staging_dir,
            _modules: modules,
            _module_pointers: module_pointers,
        }))
    }
}

fn c_string(field: &'static str, value: String) -> Result<CString, Error> {
    CString::new(value).map_err(|_| Error::InteriorNul(field))
}

fn optional_c_string(field: &'static str, value: Option<String>) -> Result<Option<CString>, Error> {
    value.map(|value| c_string(field, value)).transpose()
}

fn optional_pointer(value: &Option<CString>) -> *const c_char {
    value
        .as_ref()
        .map_or(std::ptr::null(), |value| value.as_ptr())
}

#[derive(Clone, Copy)]
struct Functions {
    setup: unsafe extern "C" fn(*mut sys::RimeTraits),
    initialize: unsafe extern "C" fn(*mut sys::RimeTraits),
    finalize: unsafe extern "C" fn(),
    create_session: unsafe extern "C" fn() -> sys::RimeSessionId,
    destroy_session: unsafe extern "C" fn(sys::RimeSessionId) -> sys::RimeBool,
    cleanup_all_sessions: unsafe extern "C" fn(),
    process_key: unsafe extern "C" fn(sys::RimeSessionId, c_int, c_int) -> sys::RimeBool,
    commit_composition: unsafe extern "C" fn(sys::RimeSessionId) -> sys::RimeBool,
    clear_composition: unsafe extern "C" fn(sys::RimeSessionId),
    get_commit: unsafe extern "C" fn(sys::RimeSessionId, *mut sys::RimeCommit) -> sys::RimeBool,
    free_commit: unsafe extern "C" fn(*mut sys::RimeCommit) -> sys::RimeBool,
    get_context: unsafe extern "C" fn(sys::RimeSessionId, *mut sys::RimeContext) -> sys::RimeBool,
    free_context: unsafe extern "C" fn(*mut sys::RimeContext) -> sys::RimeBool,
    get_status: unsafe extern "C" fn(sys::RimeSessionId, *mut sys::RimeStatus) -> sys::RimeBool,
    free_status: unsafe extern "C" fn(*mut sys::RimeStatus) -> sys::RimeBool,
    set_option: sys::SetOptionFn,
    get_option: sys::GetOptionFn,
    select_schema: sys::SelectSchemaFn,
    select_candidate_on_current_page: sys::SelectCandidateOnCurrentPageFn,
    change_page: sys::ChangePageFn,
}

impl Functions {
    unsafe fn load(api: *mut sys::RimeApi) -> Result<Self, Error> {
        let api = NonNull::new(api).ok_or(Error::NullApi)?;
        // SAFETY: the caller promises that `api` points at a live librime API
        // table; reading its first C int is valid before interpreting the rest.
        let advertised = unsafe { api.as_ptr().cast::<c_int>().read() };
        if !sys::advertised_range_available(
            advertised,
            std::mem::offset_of!(sys::RimeApi, free_status),
            size_of::<sys::FreeStatusFn>(),
        ) {
            return Err(Error::ApiTooOld {
                advertised,
                required: sys::RIME_API_REQUIRED_DATA_SIZE,
            });
        }

        // SAFETY: the range check above proves the entire committed prefix is
        // present, and the caller guarantees the table remains live.
        let raw_api = api.as_ptr();
        let api = unsafe { api.as_ref() };
        macro_rules! required {
            ($field:ident) => {
                api.$field
                    .ok_or(Error::MissingFunction(stringify!($field)))?
            };
        }

        Ok(Self {
            setup: required!(setup),
            initialize: required!(initialize),
            finalize: required!(finalize),
            create_session: required!(create_session),
            destroy_session: required!(destroy_session),
            cleanup_all_sessions: required!(cleanup_all_sessions),
            process_key: required!(process_key),
            commit_composition: required!(commit_composition),
            clear_composition: required!(clear_composition),
            get_commit: required!(get_commit),
            free_commit: required!(free_commit),
            get_context: required!(get_context),
            free_context: required!(free_context),
            get_status: required!(get_status),
            free_status: required!(free_status),
            set_option: unsafe { Self::load_set_option(raw_api, advertised) },
            get_option: unsafe { Self::load_get_option(raw_api, advertised) },
            select_schema: unsafe { Self::load_select_schema(raw_api, advertised) },
            // SAFETY: the caller supplies a live table; the helper checks each
            // complete field range before reading optional tail members.
            select_candidate_on_current_page: unsafe {
                Self::load_select_candidate(raw_api, advertised)
            },
            // SAFETY: same table/range guarantees, independently checked.
            change_page: unsafe { Self::load_change_page(raw_api, advertised) },
        })
    }

    unsafe fn load_select_candidate(
        api: *const sys::RimeApi,
        advertised: c_int,
    ) -> sys::SelectCandidateOnCurrentPageFn {
        if !sys::advertised_range_available(
            advertised,
            sys::RIME_API_SELECT_CURRENT_PAGE_OFFSET,
            size_of::<sys::SelectCandidateOnCurrentPageFn>(),
        ) {
            return None;
        }
        // SAFETY: complete typed slot is advertised. addr_of! forms no
        // reference to the extension or intervening unmodeled functions.
        unsafe {
            std::ptr::addr_of!(
                (*api.cast::<sys::RimeApiCandidateExtension>()).select_candidate_on_current_page
            )
            .read()
        }
    }

    unsafe fn load_set_option(api: *const sys::RimeApi, advertised: c_int) -> sys::SetOptionFn {
        if !sys::advertised_range_available(
            advertised,
            sys::RIME_API_SET_OPTION_OFFSET,
            size_of::<sys::SetOptionFn>(),
        ) {
            return None;
        }
        unsafe {
            std::ptr::addr_of!((*api.cast::<sys::RimeApiCandidateExtension>()).set_option).read()
        }
    }

    unsafe fn load_get_option(api: *const sys::RimeApi, advertised: c_int) -> sys::GetOptionFn {
        if !sys::advertised_range_available(
            advertised,
            sys::RIME_API_GET_OPTION_OFFSET,
            size_of::<sys::GetOptionFn>(),
        ) {
            return None;
        }
        unsafe {
            std::ptr::addr_of!((*api.cast::<sys::RimeApiCandidateExtension>()).get_option).read()
        }
    }

    unsafe fn load_select_schema(
        api: *const sys::RimeApi,
        advertised: c_int,
    ) -> sys::SelectSchemaFn {
        if !sys::advertised_range_available(
            advertised,
            sys::RIME_API_SELECT_SCHEMA_OFFSET,
            size_of::<sys::SelectSchemaFn>(),
        ) {
            return None;
        }
        unsafe {
            std::ptr::addr_of!((*api.cast::<sys::RimeApiCandidateExtension>()).select_schema).read()
        }
    }

    unsafe fn load_change_page(api: *const sys::RimeApi, advertised: c_int) -> sys::ChangePageFn {
        if !sys::advertised_range_available(
            advertised,
            sys::RIME_API_CHANGE_PAGE_OFFSET,
            size_of::<sys::ChangePageFn>(),
        ) {
            return None;
        }
        // SAFETY: complete typed slot is advertised; no full-tail reference.
        unsafe {
            std::ptr::addr_of!((*api.cast::<sys::RimeApiCandidateExtension>()).change_page).read()
        }
    }
}

/// A process-global librime instance.
///
/// The `Rc` marker makes this type `!Send + !Sync`.  Move all access through a
/// single engine actor rather than locking this value across arbitrary threads.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<mo_rime::Engine>();
/// ```
pub struct Engine {
    functions: Functions,
    prepare_resources: Option<PrepareResources>,
    _traits: Box<TraitStorage>,
    _gate: MutexGuard<'static, ()>,
    _thread_affinity: PhantomData<Rc<()>>,
    #[cfg(windows)]
    _runtime_library: Option<windows_loader::LoadedLibrary>,
}

impl Engine {
    /// Loads an explicitly configured `rime.dll` without consulting PATH or
    /// the process current directory, then initializes librime.
    #[cfg(windows)]
    pub fn load(
        config: EngineConfig,
        dll_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, Error> {
        let library = windows_loader::LoadedLibrary::load(dll_path.as_ref())?;
        // SAFETY: `library` owns the module that exports this table and is moved
        // into the Engine before this method returns.
        let api = unsafe { library.rime_api()? };
        // SAFETY: the optional versioned export belongs to this same live DLL.
        let prepare_resources = unsafe { library.prepare_resources() };
        // SAFETY: the API came from the live, explicitly loaded librime module.
        let mut engine = unsafe { Self::from_raw_api(config, api)? };
        engine.prepare_resources = prepare_resources;
        engine._runtime_library = Some(library);
        Ok(engine)
    }

    /// Initializes the linked librime selected by a `link-*` Cargo feature.
    #[cfg(any(feature = "link-dynamic", feature = "link-static"))]
    pub fn open(config: EngineConfig) -> Result<Self, Error> {
        // SAFETY: the symbol is provided by the linked library and librime owns
        // its process-lifetime API table.
        unsafe { Self::from_raw_api(config, sys::rime_get_api()) }
    }

    /// Initializes an API table supplied by a native or runtime loader.
    ///
    /// # Safety
    ///
    /// `api` must be a correctly aligned table returned by `rime_get_api` from
    /// the pinned compatible librime.  Its containing library and all function
    /// pointers must remain loaded until the returned `Engine` is dropped.
    pub unsafe fn from_raw_api(
        config: EngineConfig,
        api: *mut sys::RimeApi,
    ) -> Result<Self, Error> {
        let gate = match ENGINE_GATE.try_lock() {
            Ok(gate) => gate,
            Err(TryLockError::WouldBlock) => return Err(Error::EngineAlreadyActive),
            Err(TryLockError::Poisoned(_)) => return Err(Error::EngineGatePoisoned),
        };
        let mut traits = TraitStorage::new(config)?;
        // SAFETY: delegated to this method's caller; `Functions::load` also
        // validates that the complete prefix and every required pointer exist.
        let functions = unsafe { Functions::load(api)? };

        // All fallible Rust preparation happens before the first native call.
        // SAFETY: the function table and trait pointers meet the method's
        // contract, and TraitStorage outlives finalization.
        unsafe {
            (functions.setup)(&mut traits.raw);
            (functions.initialize)(&mut traits.raw);
        }

        Ok(Self {
            functions,
            prepare_resources: None,
            _traits: traits,
            _gate: gate,
            _thread_affinity: PhantomData,
            #[cfg(windows)]
            _runtime_library: None,
        })
    }

    pub fn create_session(&self) -> Result<Session<'_>, Error> {
        let id = self.create_session_id()?;
        Ok(Session {
            engine: self,
            id,
            active: true,
        })
    }

    fn create_session_id(&self) -> Result<sys::RimeSessionId, Error> {
        // SAFETY: Engine owns an initialized live function table and calls are
        // confined to its owning thread by the type's auto-trait markers.
        let id = unsafe { (self.functions.create_session)() };
        if id == sys::RIME_NO_SESSION {
            return Err(Error::SessionCreationFailed);
        }
        Ok(id)
    }

    fn destroy_session_id(&self, id: sys::RimeSessionId) -> Result<(), Error> {
        // SAFETY: callers own the live id and retire it after this call.
        if sys::from_rime_bool(unsafe { (self.functions.destroy_session)(id) }) {
            Ok(())
        } else {
            Err(Error::NativeCallFailed("destroy_session"))
        }
    }

    fn select_schema_id(&self, id: sys::RimeSessionId, schema: &str) -> Result<(), Error> {
        let select = self
            .functions
            .select_schema
            .ok_or(Error::MissingFunction("select_schema"))?;
        let schema = c_string("schema_id", schema.to_owned())?;
        if sys::from_rime_bool(unsafe { select(id, schema.as_ptr()) }) {
            Ok(())
        } else {
            Err(Error::NativeCallFailed("select_schema"))
        }
    }

    fn set_option_id(&self, id: sys::RimeSessionId, name: &str, value: bool) -> Result<(), Error> {
        let set = self
            .functions
            .set_option
            .ok_or(Error::MissingFunction("set_option"))?;
        let get = self
            .functions
            .get_option
            .ok_or(Error::MissingFunction("get_option"))?;
        let name = c_string("option_name", name.to_owned())?;
        unsafe { set(id, name.as_ptr(), sys::to_rime_bool(value)) };
        if sys::from_rime_bool(unsafe { get(id, name.as_ptr()) }) == value {
            Ok(())
        } else {
            Err(Error::NativeCallFailed("set_option"))
        }
    }

    fn prepare_resources_id(&self, id: sys::RimeSessionId) -> Result<(), Error> {
        let prepare = self
            .prepare_resources
            .ok_or(Error::MissingFunction("mo_rime_prepare_resources_v3"))?;
        // SAFETY: the owner keeps the DLL and session alive on the engine thread.
        // The versioned extension accepts only an empty fixed-schema session and
        // reads converters without process_key, context mutation or commit.
        if unsafe { prepare(id) } == 1 {
            Ok(())
        } else {
            Err(Error::NativeCallFailed("mo_rime_prepare_resources_v3"))
        }
    }

    fn process_key_id(&self, id: sys::RimeSessionId, keycode: c_int, modifiers: c_int) -> bool {
        // SAFETY: callers prove the id belongs to this initialized Engine.
        sys::from_rime_bool(unsafe { (self.functions.process_key)(id, keycode, modifiers) })
    }

    fn commit_composition_id(&self, id: sys::RimeSessionId) -> bool {
        // SAFETY: callers prove the id belongs to this initialized Engine.
        sys::from_rime_bool(unsafe { (self.functions.commit_composition)(id) })
    }

    fn clear_composition_id(&self, id: sys::RimeSessionId) {
        // SAFETY: callers prove the id belongs to this initialized Engine.
        unsafe { (self.functions.clear_composition)(id) }
    }

    fn select_candidate_id(&self, id: sys::RimeSessionId, index: usize) -> Result<bool, Error> {
        let select = self
            .functions
            .select_candidate_on_current_page
            .ok_or(Error::MissingFunction("select_candidate_on_current_page"))?;
        // SAFETY: id is owned by this Engine; native API validates the
        // zero-based page-local index and reports unavailable candidates.
        Ok(sys::from_rime_bool(unsafe { select(id, index) }))
    }

    fn change_page_id(&self, id: sys::RimeSessionId, backward: bool) -> Result<bool, Error> {
        let change = self
            .functions
            .change_page
            .ok_or(Error::MissingFunction("change_page"))?;
        // SAFETY: id is live and confined to the Engine's owning thread.
        Ok(sys::from_rime_bool(unsafe {
            change(id, sys::to_rime_bool(backward))
        }))
    }

    fn take_commit_id(&self, id: sys::RimeSessionId) -> Result<Option<CommitSnapshot>, Error> {
        let mut raw = sys::RimeCommit::default();
        // SAFETY: raw is correctly initialized and writable for this call.
        if !sys::from_rime_bool(unsafe { (self.functions.get_commit)(id, &mut raw) }) {
            return Ok(None);
        }
        let guard = NativeOutput::new(&mut raw, self.functions.free_commit, "RimeCommit");
        // SAFETY: a successful get_commit populated the guarded value.
        let copied = unsafe { copy_commit(guard.get()) };
        guard.release()?;
        copied.map(Some)
    }

    fn context_id(&self, id: sys::RimeSessionId) -> Result<Option<ContextSnapshot>, Error> {
        let mut raw = sys::RimeContext::default();
        // SAFETY: raw is correctly initialized and writable for this call.
        if !sys::from_rime_bool(unsafe { (self.functions.get_context)(id, &mut raw) }) {
            return Ok(None);
        }
        let guard = NativeOutput::new(&mut raw, self.functions.free_context, "RimeContext");
        // SAFETY: a successful get_context populated the guarded value.
        let copied = unsafe { copy_context(guard.get()) };
        guard.release()?;
        copied.map(Some)
    }

    fn status_id(&self, id: sys::RimeSessionId) -> Result<Option<StatusSnapshot>, Error> {
        let mut raw = sys::RimeStatus::default();
        // SAFETY: raw is correctly initialized and writable for this call.
        if !sys::from_rime_bool(unsafe { (self.functions.get_status)(id, &mut raw) }) {
            return Ok(None);
        }
        let guard = NativeOutput::new(&mut raw, self.functions.free_status, "RimeStatus");
        // SAFETY: a successful get_status populated the guarded value.
        let copied = unsafe { copy_status(guard.get()) };
        guard.release()?;
        Ok(Some(copied))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: this is the final use of the initialized process-global table.
        // cleanup_all_sessions also covers an intentionally forgotten Session.
        unsafe {
            (self.functions.cleanup_all_sessions)();
            (self.functions.finalize)();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSnapshot {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionSnapshot {
    /// Byte length reported by librime for the UTF-8 preedit.
    pub byte_length: c_int,
    /// UTF-8 byte offset reported by librime.
    pub cursor_byte_pos: c_int,
    /// UTF-8 byte offset reported by librime.
    pub selection_start_byte: c_int,
    /// UTF-8 byte offset reported by librime.
    pub selection_end_byte: c_int,
    pub preedit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSnapshot {
    pub text: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSnapshot {
    pub page_size: c_int,
    pub page_number: c_int,
    pub is_last_page: bool,
    pub highlighted_candidate_index: c_int,
    pub candidates: Vec<CandidateSnapshot>,
    pub select_keys: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSnapshot {
    pub composition: CompositionSnapshot,
    pub menu: MenuSnapshot,
    pub commit_text_preview: Option<String>,
    pub select_labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub schema_id: Option<String>,
    pub schema_name: Option<String>,
    pub is_disabled: bool,
    pub is_composing: bool,
    pub is_ascii_mode: bool,
    pub is_full_shape: bool,
    pub is_simplified: bool,
    pub is_traditional: bool,
    pub is_ascii_punct: bool,
}

pub struct Session<'engine> {
    engine: &'engine Engine,
    id: sys::RimeSessionId,
    active: bool,
}

impl Session<'_> {
    /// Reads the actual session's conversion dictionaries without sending input.
    ///
    /// Requires Mo's v3 native extension and an empty `rime_ice` session. The
    /// native boundary rejects busy/other-schema sessions; it never clears them.
    pub fn prepare_resources(&mut self) -> Result<(), Error> {
        self.engine.prepare_resources_id(self.id)
    }

    pub fn id(&self) -> sys::RimeSessionId {
        self.id
    }

    /// Selects a deployed schema for this live session.
    pub fn select_schema(&mut self, schema: &str) -> Result<(), Error> {
        self.engine.select_schema_id(self.id, schema)
    }

    /// Sets a native boolean switch and verifies its observable value.
    pub fn set_option(&mut self, name: &str, value: bool) -> Result<(), Error> {
        self.engine.set_option_id(self.id, name, value)
    }

    pub fn process_key(&mut self, keycode: c_int, modifiers: c_int) -> bool {
        self.engine.process_key_id(self.id, keycode, modifiers)
    }

    pub fn commit_composition(&mut self) -> bool {
        self.engine.commit_composition_id(self.id)
    }

    pub fn clear_composition(&mut self) {
        self.engine.clear_composition_id(self.id);
    }

    /// Selects a zero-based index on the current candidate page. False means
    /// the native engine did not select it; old APIs explicitly report missing.
    pub fn select_candidate_on_current_page(&mut self, index: usize) -> Result<bool, Error> {
        self.engine.select_candidate_id(self.id, index)
    }

    /// Changes the candidate page without synthesizing schema-specific keys.
    pub fn change_page(&mut self, backward: bool) -> Result<bool, Error> {
        self.engine.change_page_id(self.id, backward)
    }

    pub fn take_commit(&mut self) -> Result<Option<CommitSnapshot>, Error> {
        self.engine.take_commit_id(self.id)
    }

    pub fn context(&mut self) -> Result<Option<ContextSnapshot>, Error> {
        self.engine.context_id(self.id)
    }

    pub fn status(&mut self) -> Result<Option<StatusSnapshot>, Error> {
        self.engine.status_id(self.id)
    }

    pub fn close(mut self) -> Result<(), Error> {
        self.destroy()
    }

    fn destroy(&mut self) -> Result<(), Error> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        self.engine.destroy_session_id(self.id)
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

type FreeOutput<T> = unsafe extern "C" fn(*mut T) -> sys::RimeBool;

struct NativeOutput<T> {
    value: NonNull<T>,
    free: FreeOutput<T>,
    name: &'static str,
    armed: bool,
}

impl<T> NativeOutput<T> {
    fn new(value: &mut T, free: FreeOutput<T>, name: &'static str) -> Self {
        Self {
            value: NonNull::from(value),
            free,
            name,
            armed: true,
        }
    }

    fn get(&self) -> &T {
        // SAFETY: NativeOutput is created from an exclusive live reference and
        // never outlives the local native value.
        unsafe { self.value.as_ref() }
    }

    fn release(mut self) -> Result<(), Error> {
        self.armed = false;
        // SAFETY: this is the exactly-once matching free call for a successful
        // get_* operation.
        let released = unsafe { (self.free)(self.value.as_ptr()) };
        if sys::from_rime_bool(released) {
            Ok(())
        } else {
            Err(Error::NativeReleaseFailed(self.name))
        }
    }
}

impl<T> Drop for NativeOutput<T> {
    fn drop(&mut self) {
        if self.armed {
            // SAFETY: the guard still owns the matching native output.
            let _ = unsafe { (self.free)(self.value.as_ptr()) };
        }
    }
}

unsafe fn copy_commit(raw: &sys::RimeCommit) -> Result<CommitSnapshot, Error> {
    if raw.text.is_null() {
        return Err(Error::NullArray {
            field: "commit.text",
            count: 1,
        });
    }
    // SAFETY: get_commit promises a live NUL-terminated string until free_commit.
    Ok(CommitSnapshot {
        text: unsafe { copy_string(raw.text) },
    })
}

unsafe fn copy_context(raw: &sys::RimeContext) -> Result<ContextSnapshot, Error> {
    let candidate_count = checked_count(
        "context.menu.num_candidates",
        raw.menu.num_candidates,
        MAX_CANDIDATES_PER_PAGE,
    )?;
    let page_size = checked_count(
        "context.menu.page_size",
        raw.menu.page_size,
        MAX_SELECT_LABELS,
    )?;

    if candidate_count != 0 && raw.menu.candidates.is_null() {
        return Err(Error::NullArray {
            field: "context.menu.candidates",
            count: candidate_count,
        });
    }
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(candidate_count)
        .map_err(|_| Error::AllocationFailed("context.menu.candidates"))?;
    if candidate_count != 0 {
        // SAFETY: count was bounded and the non-NULL array comes from librime.
        let raw_candidates =
            unsafe { std::slice::from_raw_parts(raw.menu.candidates, candidate_count) };
        candidates.extend(raw_candidates.iter().map(|candidate| CandidateSnapshot {
            // SAFETY: candidate strings live until free_context.  NULL is
            // represented as an empty candidate, matching upstream frontends.
            text: unsafe { copy_optional_string(candidate.text) }.unwrap_or_default(),
            // SAFETY: same lifetime as candidate.text.
            comment: unsafe { copy_optional_string(candidate.comment) },
        }));
    }

    let mut select_labels = Vec::new();
    if !raw.select_labels.is_null() && page_size != 0 {
        select_labels
            .try_reserve_exact(page_size)
            .map_err(|_| Error::AllocationFailed("context.select_labels"))?;
        // SAFETY: librime exposes page_size entries when select_labels is set.
        let labels = unsafe { std::slice::from_raw_parts(raw.select_labels, page_size) };
        select_labels.extend(
            labels
                .iter()
                // SAFETY: each optional label lives until free_context.
                .map(|label| unsafe { copy_optional_string(*label) }.unwrap_or_default()),
        );
    }

    Ok(ContextSnapshot {
        composition: CompositionSnapshot {
            byte_length: raw.composition.length,
            cursor_byte_pos: raw.composition.cursor_pos,
            selection_start_byte: raw.composition.sel_start,
            selection_end_byte: raw.composition.sel_end,
            // SAFETY: optional preedit lives until free_context.
            preedit: unsafe { copy_optional_string(raw.composition.preedit) }.unwrap_or_default(),
        },
        menu: MenuSnapshot {
            page_size: raw.menu.page_size,
            page_number: raw.menu.page_no,
            is_last_page: sys::from_rime_bool(raw.menu.is_last_page),
            highlighted_candidate_index: raw.menu.highlighted_candidate_index,
            candidates,
            // SAFETY: optional select_keys lives until free_context.
            select_keys: unsafe { copy_optional_string(raw.menu.select_keys) },
        },
        // SAFETY: optional preview lives until free_context.
        commit_text_preview: unsafe { copy_optional_string(raw.commit_text_preview) },
        select_labels,
    })
}

unsafe fn copy_status(raw: &sys::RimeStatus) -> StatusSnapshot {
    StatusSnapshot {
        // SAFETY: status strings live until free_status.
        schema_id: unsafe { copy_optional_string(raw.schema_id) },
        // SAFETY: status strings live until free_status.
        schema_name: unsafe { copy_optional_string(raw.schema_name) },
        is_disabled: sys::from_rime_bool(raw.is_disabled),
        is_composing: sys::from_rime_bool(raw.is_composing),
        is_ascii_mode: sys::from_rime_bool(raw.is_ascii_mode),
        is_full_shape: sys::from_rime_bool(raw.is_full_shape),
        is_simplified: sys::from_rime_bool(raw.is_simplified),
        is_traditional: sys::from_rime_bool(raw.is_traditional),
        is_ascii_punct: sys::from_rime_bool(raw.is_ascii_punct),
    }
}

fn checked_count(field: &'static str, value: c_int, maximum: usize) -> Result<usize, Error> {
    let value = usize::try_from(value).map_err(|_| Error::InvalidCount { field, value })?;
    if value > maximum {
        return Err(Error::InvalidCount {
            field,
            value: c_int::try_from(value).unwrap_or(c_int::MAX),
        });
    }
    Ok(value)
}

unsafe fn copy_string(pointer: *const c_char) -> String {
    // SAFETY: caller guarantees a live NUL-terminated C string.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

unsafe fn copy_optional_string(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        None
    } else {
        // SAFETY: caller forwards the native string lifetime guarantee.
        Some(unsafe { copy_string(pointer) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mo_domain::SessionOptions;
    use mo_engine::EngineBackend;
    use std::ptr;

    static TEST_SERIAL: Mutex<()> = Mutex::new(());
    static FAKE: Mutex<FakeState> = Mutex::new(FakeState::new());

    #[derive(Debug)]
    struct FakeState {
        calls: Vec<&'static str>,
        traits_are_valid: bool,
        malformed_context: bool,
        valid_utf8_context: bool,
        last_key: Option<(c_int, c_int)>,
        last_candidate: Option<usize>,
        last_page_backward: Option<bool>,
        last_option: Option<bool>,
    }

    impl FakeState {
        const fn new() -> Self {
            Self {
                calls: Vec::new(),
                traits_are_valid: false,
                malformed_context: false,
                valid_utf8_context: false,
                last_key: None,
                last_candidate: None,
                last_page_backward: None,
                last_option: None,
            }
        }

        fn reset(&mut self) {
            self.calls.clear();
            self.traits_are_valid = false;
            self.malformed_context = false;
            self.valid_utf8_context = false;
            self.last_key = None;
            self.last_candidate = None;
            self.last_page_backward = None;
            self.last_option = None;
        }
    }

    fn record(call: &'static str) {
        if let Ok(mut state) = FAKE.lock() {
            state.calls.push(call);
        }
    }

    unsafe extern "C" fn fake_setup(traits: *mut sys::RimeTraits) {
        let valid = if traits.is_null() {
            false
        } else {
            // SAFETY: test passes its live TraitStorage pointer.
            let traits = unsafe { &*traits };
            traits.data_size == sys::rime_struct_data_size::<sys::RimeTraits>()
                && !traits.shared_data_dir.is_null()
                && !traits.user_data_dir.is_null()
                && !traits.modules.is_null()
        };
        if let Ok(mut state) = FAKE.lock() {
            state.calls.push("setup");
            state.traits_are_valid = valid;
        }
    }

    unsafe extern "C" fn fake_initialize(_: *mut sys::RimeTraits) {
        record("initialize");
    }

    unsafe extern "C" fn fake_finalize() {
        record("finalize");
    }

    unsafe extern "C" fn fake_create_session() -> sys::RimeSessionId {
        record("create_session");
        42
    }

    unsafe extern "C" fn fake_destroy_session(_: sys::RimeSessionId) -> sys::RimeBool {
        record("destroy_session");
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_cleanup_all_sessions() {
        record("cleanup_all_sessions");
    }

    unsafe extern "C" fn fake_process_key(
        _: sys::RimeSessionId,
        keycode: c_int,
        modifiers: c_int,
    ) -> sys::RimeBool {
        if let Ok(mut state) = FAKE.lock() {
            state.calls.push("process_key");
            state.last_key = Some((keycode, modifiers));
        }
        sys::to_rime_bool(keycode == 65)
    }

    unsafe extern "C" fn fake_commit_composition(_: sys::RimeSessionId) -> sys::RimeBool {
        record("commit_composition");
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_clear_composition(_: sys::RimeSessionId) {
        record("clear_composition");
    }

    unsafe extern "C" fn fake_select_candidate(
        _: sys::RimeSessionId,
        index: usize,
    ) -> sys::RimeBool {
        let mut state = FAKE.lock().unwrap();
        state.calls.push("select_candidate_on_current_page");
        state.last_candidate = Some(index);
        sys::to_rime_bool(index < 2)
    }

    unsafe extern "C" fn fake_change_page(
        _: sys::RimeSessionId,
        backward: sys::RimeBool,
    ) -> sys::RimeBool {
        let mut state = FAKE.lock().unwrap();
        state.calls.push("change_page");
        state.last_page_backward = Some(sys::from_rime_bool(backward));
        sys::to_rime_bool(!sys::from_rime_bool(backward))
    }

    unsafe extern "C" fn fake_select_schema(
        _: sys::RimeSessionId,
        _: *const c_char,
    ) -> sys::RimeBool {
        record("select_schema");
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_set_option(
        _: sys::RimeSessionId,
        _: *const c_char,
        value: sys::RimeBool,
    ) {
        let mut state = FAKE.lock().unwrap();
        state.calls.push("set_option");
        state.last_option = Some(sys::from_rime_bool(value));
    }

    unsafe extern "C" fn fake_get_option(_: sys::RimeSessionId, _: *const c_char) -> sys::RimeBool {
        let mut state = FAKE.lock().unwrap();
        state.calls.push("get_option");
        sys::to_rime_bool(state.last_option.unwrap_or(false))
    }

    unsafe extern "C" fn fake_get_commit(
        _: sys::RimeSessionId,
        output: *mut sys::RimeCommit,
    ) -> sys::RimeBool {
        record("get_commit");
        if output.is_null() {
            return sys::RIME_FALSE;
        }
        // SAFETY: wrapper provides a live initialized output.
        unsafe { (*output).text = owned_c_string(b"\xe4\xbd\xa0\xe5\xa5\xbd\0") };
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_free_commit(output: *mut sys::RimeCommit) -> sys::RimeBool {
        record("free_commit");
        if output.is_null() {
            return sys::RIME_FALSE;
        }
        // SAFETY: fake_get_commit allocated this exact pointer once.
        unsafe {
            free_c_string((*output).text);
            (*output).text = ptr::null_mut();
        }
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_get_context(
        _: sys::RimeSessionId,
        output: *mut sys::RimeContext,
    ) -> sys::RimeBool {
        record("get_context");
        if output.is_null() {
            return sys::RIME_FALSE;
        }
        let (malformed, valid_utf8) = FAKE
            .lock()
            .map(|state| (state.malformed_context, state.valid_utf8_context))
            .unwrap_or((false, false));
        // SAFETY: wrapper provides a live initialized output.
        let output = unsafe { &mut *output };
        if valid_utf8 {
            output.composition.length = 5;
            output.composition.cursor_pos = 5;
            output.composition.preedit = unsafe { owned_c_string(b"nihao\0") };
        } else {
            output.composition.length = 6;
            output.composition.cursor_pos = 6;
            // Include invalid UTF-8 to prove conversion is owned and non-UB.
            output.composition.preedit = unsafe { owned_c_string(b"ni\xffhao\0") };
        }
        output.menu.page_size = 2;
        output.menu.page_no = 0;
        output.menu.is_last_page = -1;
        output.menu.highlighted_candidate_index = 0;
        output.menu.num_candidates = 2;
        output.menu.select_keys = unsafe { owned_c_string(b"12\0") };
        output.commit_text_preview = unsafe { owned_c_string(b"preview\0") };

        if !malformed {
            let candidates = vec![
                sys::RimeCandidate {
                    text: unsafe { owned_c_string(b"one\0") },
                    comment: ptr::null_mut(),
                    reserved: ptr::null_mut(),
                },
                sys::RimeCandidate {
                    text: unsafe { owned_c_string(b"two\0") },
                    comment: unsafe { owned_c_string(b"comment\0") },
                    reserved: ptr::null_mut(),
                },
            ]
            .into_boxed_slice();
            output.menu.candidates = Box::into_raw(candidates).cast::<sys::RimeCandidate>();
        }

        let labels = vec![unsafe { owned_c_string(b"1.\0") }, unsafe {
            owned_c_string(b"2.\0")
        }]
        .into_boxed_slice();
        output.select_labels = Box::into_raw(labels).cast::<*mut c_char>();
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_free_context(output: *mut sys::RimeContext) -> sys::RimeBool {
        record("free_context");
        if output.is_null() {
            return sys::RIME_FALSE;
        }
        // SAFETY: allocations below were created by fake_get_context and are
        // recovered exactly once using their recorded C counts.
        let output = unsafe { &mut *output };
        unsafe {
            free_c_string(output.composition.preedit);
            free_c_string(output.menu.select_keys);
            free_c_string(output.commit_text_preview);
        }
        output.composition.preedit = ptr::null_mut();
        output.menu.select_keys = ptr::null_mut();
        output.commit_text_preview = ptr::null_mut();

        if !output.menu.candidates.is_null() {
            let count = usize::try_from(output.menu.num_candidates).unwrap_or_default();
            // SAFETY: pointer/count reconstruct the allocation from get_context.
            let candidates = unsafe {
                Box::from_raw(ptr::slice_from_raw_parts_mut(output.menu.candidates, count))
            };
            for candidate in &*candidates {
                unsafe {
                    free_c_string(candidate.text);
                    free_c_string(candidate.comment);
                }
            }
            drop(candidates);
            output.menu.candidates = ptr::null_mut();
        }

        if !output.select_labels.is_null() {
            let count = usize::try_from(output.menu.page_size).unwrap_or_default();
            // SAFETY: pointer/count reconstruct the allocation from get_context.
            let labels = unsafe {
                Box::from_raw(ptr::slice_from_raw_parts_mut(output.select_labels, count))
            };
            for label in &*labels {
                unsafe { free_c_string(*label) };
            }
            drop(labels);
            output.select_labels = ptr::null_mut();
        }
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_get_status(
        _: sys::RimeSessionId,
        output: *mut sys::RimeStatus,
    ) -> sys::RimeBool {
        record("get_status");
        if output.is_null() {
            return sys::RIME_FALSE;
        }
        // SAFETY: wrapper provides a live initialized output.
        unsafe {
            (*output).schema_id = owned_c_string(b"mo_pinyin\0");
            (*output).schema_name = owned_c_string(b"Mo\0");
            (*output).is_composing = sys::RIME_TRUE;
            (*output).is_simplified = 7;
        }
        sys::RIME_TRUE
    }

    unsafe extern "C" fn fake_free_status(output: *mut sys::RimeStatus) -> sys::RimeBool {
        record("free_status");
        if output.is_null() {
            return sys::RIME_FALSE;
        }
        // SAFETY: fake_get_status allocated these exact pointers once.
        unsafe {
            free_c_string((*output).schema_id);
            free_c_string((*output).schema_name);
            (*output).schema_id = ptr::null_mut();
            (*output).schema_name = ptr::null_mut();
        }
        sys::RIME_TRUE
    }

    unsafe fn owned_c_string(bytes_with_nul: &[u8]) -> *mut c_char {
        // Test constants are all explicitly NUL-terminated with no earlier NUL.
        // SAFETY: upheld by each hard-coded caller in this module.
        unsafe { CString::from_vec_with_nul_unchecked(bytes_with_nul.to_vec()) }.into_raw()
    }

    unsafe fn free_c_string(pointer: *mut c_char) {
        if !pointer.is_null() {
            // SAFETY: pointer came from CString::into_raw in owned_c_string.
            drop(unsafe { CString::from_raw(pointer) });
        }
    }

    fn fake_api() -> sys::RimeApi {
        sys::RimeApi {
            setup: Some(fake_setup),
            initialize: Some(fake_initialize),
            finalize: Some(fake_finalize),
            create_session: Some(fake_create_session),
            destroy_session: Some(fake_destroy_session),
            cleanup_all_sessions: Some(fake_cleanup_all_sessions),
            process_key: Some(fake_process_key),
            commit_composition: Some(fake_commit_composition),
            clear_composition: Some(fake_clear_composition),
            get_commit: Some(fake_get_commit),
            free_commit: Some(fake_free_commit),
            get_context: Some(fake_get_context),
            free_context: Some(fake_free_context),
            get_status: Some(fake_get_status),
            free_status: Some(fake_free_status),
            ..sys::RimeApi::default()
        }
    }

    fn fake_candidate_api() -> sys::RimeApiCandidateExtension {
        sys::RimeApiCandidateExtension {
            prefix: sys::RimeApi {
                data_size: sys::RIME_API_CANDIDATE_DATA_SIZE,
                ..fake_api()
            },
            select_candidate_on_current_page: Some(fake_select_candidate),
            change_page: Some(fake_change_page),
            select_schema: Some(fake_select_schema),
            set_option: Some(fake_set_option),
            get_option: Some(fake_get_option),
            ..sys::RimeApiCandidateExtension::default()
        }
    }

    #[test]
    fn optional_candidate_slots_require_complete_advertised_fields() {
        let mut api = fake_candidate_api();
        // Keep the raw pointer derived from the full allocation, not a
        // reference to only the prefix subobject.
        let select_end = sys::RIME_API_SELECT_CURRENT_PAGE_OFFSET
            + size_of::<sys::SelectCandidateOnCurrentPageFn>()
            - size_of::<c_int>();
        api.prefix.data_size = select_end as c_int - 1;
        // SAFETY: live test allocation/table, with intentionally reduced size.
        let functions = unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }.unwrap();
        assert!(functions.select_candidate_on_current_page.is_none());
        assert!(functions.change_page.is_none());
        api.prefix.data_size += 1;
        // SAFETY: select field is now exactly advertised; page remains absent.
        let functions = unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }.unwrap();
        assert!(functions.select_candidate_on_current_page.is_some());
        assert!(functions.change_page.is_none());
        api.prefix.data_size = sys::RIME_API_CANDIDATE_DATA_SIZE - 1;
        // SAFETY: last byte of page slot is intentionally outside the range.
        assert!(
            unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }
                .unwrap()
                .change_page
                .is_none()
        );
        api.prefix.data_size += 1;
        api.change_page = None;
        // SAFETY: full slot is advertised but null; it remains unavailable.
        assert!(
            unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }
                .unwrap()
                .change_page
                .is_none()
        );
    }

    #[test]
    fn schema_and_option_slots_are_checked_independently() {
        let mut api = fake_candidate_api();
        api.prefix.data_size = sys::RIME_API_SET_OPTION_OFFSET as c_int
            + size_of::<sys::SetOptionFn>() as c_int
            - size_of::<c_int>() as c_int
            - 1;
        let loaded = unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }.unwrap();
        assert!(loaded.set_option.is_none());
        assert!(loaded.get_option.is_none());
        assert!(loaded.select_schema.is_none());
        api.prefix.data_size += 1;
        let loaded = unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }.unwrap();
        assert!(loaded.set_option.is_some());
        assert!(loaded.get_option.is_none());
        api.prefix.data_size = sys::RIME_API_CANDIDATE_DATA_SIZE;
        let loaded = unsafe { Functions::load(std::ptr::addr_of_mut!(api).cast()) }.unwrap();
        assert!(loaded.get_option.is_some());
        assert!(loaded.select_schema.is_some());
    }

    #[test]
    fn backend_configures_session_and_reclaims_failed_configuration() {
        let _serial = TEST_SERIAL.lock().unwrap();
        FAKE.lock().unwrap().reset();
        let mut api = fake_candidate_api();
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api.prefix) }
                .unwrap();
        let mut backend = RimeBackend::new(engine);
        let options = SessionOptions::new()
            .with_schema("double_pinyin_flypy")
            .with_option("traditionalization", true);
        let session = backend.create_session(options).unwrap();
        backend.destroy_session(session).unwrap();
        assert_eq!(FAKE.lock().unwrap().last_option, Some(true));
        assert_eq!(
            &FAKE.lock().unwrap().calls[2..7],
            &[
                "create_session",
                "select_schema",
                "set_option",
                "get_option",
                "destroy_session"
            ]
        );
        assert!(matches!(
            backend.create_session(SessionOptions::new().with_schema("unknown")),
            Err(RimeBackendError::UnsupportedSessionOptions)
        ));
        drop(backend);

        FAKE.lock().unwrap().reset();
        let mut old = fake_api();
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut old) }.unwrap();
        let mut backend = RimeBackend::new(engine);
        assert!(matches!(
            backend.create_session(SessionOptions::new().with_schema("rime_ice")),
            Err(RimeBackendError::Native(Error::MissingFunction(
                "select_schema"
            )))
        ));
        assert_eq!(
            &FAKE.lock().unwrap().calls[2..4],
            &["create_session", "destroy_session"]
        );
    }

    #[test]
    fn older_api_keeps_key_input_but_reports_missing_candidate_commands() {
        let _serial = TEST_SERIAL.lock().unwrap();
        FAKE.lock().unwrap().reset();
        let mut api = fake_api();
        // SAFETY: base-prefix table remains live throughout Engine ownership.
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }.unwrap();
        let mut session = engine.create_session().unwrap();
        assert!(session.process_key(65, 0));
        assert_eq!(
            session.select_candidate_on_current_page(0),
            Err(Error::MissingFunction("select_candidate_on_current_page"))
        );
        assert_eq!(
            session.change_page(false),
            Err(Error::MissingFunction("change_page"))
        );
        assert!(FAKE.lock().unwrap().last_candidate.is_none());
    }

    #[test]
    fn resource_anchor_receives_no_input_and_outlives_distinct_frontend_sessions() {
        use mo_domain::{EngineCommand, KeyEvent, SessionOptions};
        use mo_engine::EngineBackend;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(1);
        static EVENTS: Mutex<Vec<(&str, usize)>> = Mutex::new(Vec::new());
        unsafe extern "C" fn create() -> sys::RimeSessionId {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            EVENTS.lock().unwrap().push(("create", id));
            id
        }
        unsafe extern "C" fn destroy(id: sys::RimeSessionId) -> sys::RimeBool {
            EVENTS.lock().unwrap().push(("destroy", id));
            sys::RIME_TRUE
        }
        unsafe extern "C" fn process(
            id: sys::RimeSessionId,
            key: c_int,
            mask: c_int,
        ) -> sys::RimeBool {
            EVENTS.lock().unwrap().push(("key", id));
            // SAFETY: delegate to the same test-only callback/table contract.
            unsafe { fake_process_key(id, key, mask) }
        }
        let _serial = TEST_SERIAL.lock().unwrap();
        NEXT.store(1, Ordering::Relaxed);
        EVENTS.lock().unwrap().clear();
        FAKE.lock().unwrap().reset();
        FAKE.lock().unwrap().valid_utf8_context = true;
        let mut api = fake_api();
        api.create_session = Some(create);
        api.destroy_session = Some(destroy);
        api.process_key = Some(process);
        // SAFETY: initialized table/callbacks remain live until backend drop.
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }.unwrap();
        let mut backend = RimeBackend::with_resource_anchor(engine).unwrap();
        for _ in 0..2 {
            let mut session = backend.create_session(SessionOptions::new()).unwrap();
            backend
                .apply(&mut session, &EngineCommand::Key(KeyEvent::text('A')))
                .unwrap();
            backend.destroy_session(session).unwrap();
        }
        assert_eq!(
            *EVENTS.lock().unwrap(),
            [
                ("create", 1),
                ("create", 2),
                ("key", 2),
                ("destroy", 2),
                ("create", 3),
                ("key", 3),
                ("destroy", 3)
            ]
        );
        drop(backend);
        assert_eq!(EVENTS.lock().unwrap().last(), Some(&("destroy", 1)));
        let calls = FAKE.lock().unwrap().calls.clone();
        assert_eq!(
            &calls[calls.len() - 2..],
            ["cleanup_all_sessions", "finalize"]
        );
    }

    #[test]
    fn resource_anchor_creation_failure_finalizes_without_dispatching() {
        unsafe extern "C" fn reject() -> sys::RimeSessionId {
            record("create_session");
            0
        }
        let _serial = TEST_SERIAL.lock().unwrap();
        FAKE.lock().unwrap().reset();
        let mut api = fake_api();
        api.create_session = Some(reject);
        // SAFETY: initialized table/callbacks remain live through error cleanup.
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }.unwrap();
        assert!(matches!(
            RimeBackend::with_resource_anchor(engine),
            Err(Error::SessionCreationFailed)
        ));
        assert_eq!(
            FAKE.lock().unwrap().calls,
            [
                "setup",
                "initialize",
                "create_session",
                "cleanup_all_sessions",
                "finalize"
            ]
        );
    }

    #[test]
    fn prepared_anchor_requires_extension_without_creating_a_session() {
        let _serial = TEST_SERIAL.lock().unwrap();
        FAKE.lock().unwrap().reset();
        let mut api = fake_api();
        // SAFETY: the fake table stays live through failure/finalization.
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }.unwrap();
        assert!(matches!(
            RimeBackend::with_prepared_resources(engine),
            Err(Error::MissingFunction("mo_rime_prepare_resources_v3"))
        ));
        assert_eq!(
            FAKE.lock().unwrap().calls,
            ["setup", "initialize", "cleanup_all_sessions", "finalize"]
        );
    }

    #[test]
    fn prepared_anchor_rejects_failure_and_noncanonical_returns_then_reclaims() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static RESULT: AtomicI32 = AtomicI32::new(0);
        unsafe extern "C" fn prepare(id: sys::RimeSessionId) -> c_int {
            assert_eq!(id, 42);
            record("prepare_resources");
            RESULT.load(Ordering::Relaxed)
        }
        let _serial = TEST_SERIAL.lock().unwrap();
        for result in [0, -1, 2] {
            RESULT.store(result, Ordering::Relaxed);
            FAKE.lock().unwrap().reset();
            let mut api = fake_api();
            // SAFETY: all test callbacks/table outlive this engine.
            let mut engine =
                unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }
                    .unwrap();
            engine.prepare_resources = Some(prepare);
            assert!(matches!(
                RimeBackend::with_prepared_resources(engine),
                Err(Error::NativeCallFailed("mo_rime_prepare_resources_v3"))
            ));
            assert_eq!(
                FAKE.lock().unwrap().calls,
                [
                    "setup",
                    "initialize",
                    "create_session",
                    "prepare_resources",
                    "destroy_session",
                    "cleanup_all_sessions",
                    "finalize"
                ]
            );
        }
    }

    #[test]
    fn prepared_anchor_success_reads_only_resources_and_remains_owned_until_drop() {
        unsafe extern "C" fn prepare(id: sys::RimeSessionId) -> c_int {
            assert_eq!(id, 42);
            record("prepare_resources");
            1
        }
        let _serial = TEST_SERIAL.lock().unwrap();
        FAKE.lock().unwrap().reset();
        let mut api = fake_api();
        // SAFETY: all test callbacks/table remain live until backend teardown.
        let mut engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }.unwrap();
        engine.prepare_resources = Some(prepare);
        let backend = RimeBackend::with_prepared_resources(engine).unwrap();
        assert_eq!(
            FAKE.lock().unwrap().calls,
            ["setup", "initialize", "create_session", "prepare_resources"]
        );
        drop(backend);
        assert_eq!(
            FAKE.lock().unwrap().calls,
            [
                "setup",
                "initialize",
                "create_session",
                "prepare_resources",
                "destroy_session",
                "cleanup_all_sessions",
                "finalize"
            ]
        );
    }

    #[test]
    fn candidate_commands_route_through_actor_and_preserve_native_false() {
        use mo_domain::{EngineCommand, SessionOptions};
        use mo_engine::EngineActor;

        let _serial = TEST_SERIAL.lock().unwrap();
        FAKE.lock().unwrap().reset();
        FAKE.lock().unwrap().valid_utf8_context = true;
        let mut api = fake_candidate_api();
        let raw = std::ptr::addr_of_mut!(api).cast::<sys::RimeApi>();
        // SAFETY: complete fake table and callbacks remain live until drop.
        let engine =
            unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), raw) }.unwrap();
        let mut actor = EngineActor::new(RimeBackend::new(engine));
        let token = actor.create_session(SessionOptions::new()).unwrap();
        let selected = actor
            .dispatch(token, EngineCommand::SelectCandidate { index: 1 })
            .unwrap();
        assert!(selected.handled);
        assert_eq!(selected.commit.as_deref(), Some("你好"));
        assert_eq!(FAKE.lock().unwrap().last_candidate, Some(1));
        let missing = actor
            .dispatch(token, EngineCommand::SelectCandidate { index: u32::MAX })
            .unwrap();
        assert!(!missing.handled);
        assert_eq!(FAKE.lock().unwrap().last_candidate, Some(u32::MAX as usize));
        let next = actor
            .dispatch(token, EngineCommand::ChangePage { backward: false })
            .unwrap();
        assert!(next.handled);
        assert_eq!(FAKE.lock().unwrap().last_page_backward, Some(false));
        let previous = actor
            .dispatch(token, EngineCommand::ChangePage { backward: true })
            .unwrap();
        assert!(!previous.handled);
        assert_eq!(FAKE.lock().unwrap().last_page_backward, Some(true));
        assert!(previous.revision > selected.revision);
    }

    #[test]
    fn snapshots_are_owned_and_native_outputs_are_released_exactly_once() {
        let _serial = TEST_SERIAL.lock().expect("test serialization lock");
        FAKE.lock().expect("fake state").reset();
        let mut api = fake_api();
        let config = EngineConfig::new("shared", "user");

        // SAFETY: the fake table remains live until Engine is dropped.
        let engine = unsafe { Engine::from_raw_api(config, &mut api) }.expect("engine");
        let mut session = engine.create_session().expect("session");
        assert_eq!(session.id(), 42);
        assert!(session.process_key(65, 0));
        assert!(!session.process_key(66, 0));
        assert!(session.commit_composition());
        session.clear_composition();

        let commit = session.take_commit().expect("commit call").expect("commit");
        assert_eq!(commit.text, "你好");

        let context = session.context().expect("context call").expect("context");
        assert_eq!(context.composition.preedit, "ni\u{fffd}hao");
        assert_eq!(context.menu.candidates[1].text, "two");
        assert_eq!(
            context.menu.candidates[1].comment.as_deref(),
            Some("comment")
        );
        assert_eq!(context.select_labels, ["1.", "2."]);
        assert!(context.menu.is_last_page);

        let status = session.status().expect("status call").expect("status");
        assert_eq!(status.schema_id.as_deref(), Some("mo_pinyin"));
        assert!(status.is_composing);
        assert!(status.is_simplified);

        session.close().expect("destroy session");
        drop(engine);

        let state = FAKE.lock().expect("fake state");
        assert!(state.traits_are_valid);
        for paired_call in ["free_commit", "free_context", "free_status"] {
            assert_eq!(
                state
                    .calls
                    .iter()
                    .filter(|call| **call == paired_call)
                    .count(),
                1,
                "{paired_call} must run exactly once"
            );
        }
        assert_eq!(
            &state.calls[state.calls.len() - 3..],
            ["destroy_session", "cleanup_all_sessions", "finalize"]
        );

        // All native strings were freed before these owned Rust values are read.
        assert_eq!(commit.text, "你好");
        assert_eq!(context.menu.candidates[0].text, "one");
        assert_eq!(status.schema_name.as_deref(), Some("Mo"));
    }

    #[test]
    fn rime_backend_routes_actor_commands_and_projects_owned_output() {
        use mo_domain::{EngineCommand, KeyEvent, KeyModifiers, SessionOptions};
        use mo_engine::EngineActor;

        let _serial = TEST_SERIAL.lock().expect("test serialization lock");
        FAKE.lock().expect("fake state").reset();
        FAKE.lock().expect("fake state").valid_utf8_context = true;
        let mut api = fake_api();
        // SAFETY: the fake table remains live until the actor and backend drop.
        let engine = unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }
            .expect("engine");
        let mut actor = EngineActor::new(RimeBackend::new(engine));
        let token = actor.create_session(SessionOptions::new()).unwrap();
        let snapshot = actor
            .dispatch(
                token,
                EngineCommand::Key(
                    KeyEvent::pressed(65)
                        .with_modifiers(KeyModifiers::SHIFT | KeyModifiers::CONTROL),
                ),
            )
            .unwrap();

        assert!(snapshot.handled);
        assert_eq!(snapshot.commit.as_deref(), Some("你好"));
        assert_eq!(snapshot.composition.unwrap().preedit(), "nihao");
        assert_eq!(snapshot.candidates.len(), 2);
        assert_eq!(snapshot.candidates[1].text, "two");
        assert_eq!(snapshot.candidates[1].comment.as_deref(), Some("comment"));
        assert_eq!(snapshot.candidates[1].label.as_deref(), Some("2."));
        assert_eq!(snapshot.status.schema_id, "mo_pinyin");
        assert!(snapshot.status.composing);

        actor.destroy_session(token).unwrap();
        drop(actor);
        let state = FAKE.lock().expect("fake state");
        assert_eq!(state.last_key, Some((65, 0x0001 | 0x0004)));
        assert!(state.calls.contains(&"destroy_session"));
    }

    #[test]
    fn malformed_snapshot_still_runs_matching_free() {
        let _serial = TEST_SERIAL.lock().expect("test serialization lock");
        FAKE.lock().expect("fake state").reset();
        FAKE.lock().expect("fake state").malformed_context = true;
        let mut api = fake_api();
        // SAFETY: the fake table remains live until Engine is dropped.
        let engine = unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) }
            .expect("engine");
        let mut session = engine.create_session().expect("session");

        assert_eq!(
            session.context(),
            Err(Error::NullArray {
                field: "context.menu.candidates",
                count: 2,
            })
        );
        drop(session);
        drop(engine);

        let state = FAKE.lock().expect("fake state");
        assert_eq!(
            state
                .calls
                .iter()
                .filter(|call| **call == "free_context")
                .count(),
            1
        );
    }

    #[test]
    fn old_or_incomplete_api_is_rejected_before_setup() {
        let _serial = TEST_SERIAL.lock().expect("test serialization lock");
        FAKE.lock().expect("fake state").reset();
        let mut api = fake_api();
        api.data_size = sys::RIME_API_REQUIRED_DATA_SIZE - 1;

        // SAFETY: this is a live fake table; the test intentionally advertises
        // a shorter compatible prefix.
        let result = unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) };
        assert!(matches!(result, Err(Error::ApiTooOld { .. })));
        assert!(FAKE.lock().expect("fake state").calls.is_empty());

        let mut api = fake_api();
        api.get_context = None;
        // SAFETY: this is a live fake table with one intentionally absent slot.
        let result = unsafe { Engine::from_raw_api(EngineConfig::new("shared", "user"), &mut api) };
        assert!(matches!(result, Err(Error::MissingFunction("get_context"))));
        assert!(FAKE.lock().expect("fake state").calls.is_empty());
    }

    #[test]
    fn interior_nul_is_rejected_before_native_setup() {
        let _serial = TEST_SERIAL.lock().expect("test serialization lock");
        FAKE.lock().expect("fake state").reset();
        let mut api = fake_api();
        // SAFETY: the fake table remains live for this failed construction.
        let result =
            unsafe { Engine::from_raw_api(EngineConfig::new("bad\0path", "user"), &mut api) };
        assert_eq!(result.err(), Some(Error::InteriorNul("shared_data_dir")));
        assert!(FAKE.lock().expect("fake state").calls.is_empty());
    }

    #[test]
    fn default_config_loads_the_lua_module_required_by_rime_ice() {
        let config = EngineConfig::new("shared", "user");
        assert_eq!(config.modules, ["default", "lua"]);
    }
}
