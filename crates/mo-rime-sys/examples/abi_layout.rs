use mo_rime_sys::*;
use std::mem::{align_of, offset_of, size_of};

macro_rules! type_layout {
    ($name:literal, $ty:ty) => {
        println!(concat!($name, ".size={}"), size_of::<$ty>());
        println!(concat!($name, ".align={}"), align_of::<$ty>());
    };
}

macro_rules! field_offset {
    ($name:literal, $ty:ty, $field:ident) => {
        println!(
            concat!($name, ".offset.", stringify!($field), "={}"),
            offset_of!($ty, $field)
        );
    };
}

fn main() {
    println!("abi.pointer_bits={}", size_of::<*const ()>() * 8);
    println!("abi.bool_size={}", size_of::<RimeBool>());
    println!("abi.session_size={}", size_of::<RimeSessionId>());

    type_layout!("RimeTraits", RimeTraits);
    field_offset!("RimeTraits", RimeTraits, data_size);
    field_offset!("RimeTraits", RimeTraits, shared_data_dir);
    field_offset!("RimeTraits", RimeTraits, modules);
    field_offset!("RimeTraits", RimeTraits, min_log_level);
    field_offset!("RimeTraits", RimeTraits, staging_dir);

    type_layout!("RimeComposition", RimeComposition);
    field_offset!("RimeComposition", RimeComposition, preedit);
    type_layout!("RimeCandidate", RimeCandidate);
    field_offset!("RimeCandidate", RimeCandidate, reserved);
    type_layout!("RimeMenu", RimeMenu);
    field_offset!("RimeMenu", RimeMenu, candidates);
    field_offset!("RimeMenu", RimeMenu, select_keys);
    type_layout!("RimeCommit", RimeCommit);
    field_offset!("RimeCommit", RimeCommit, text);
    type_layout!("RimeContext", RimeContext);
    field_offset!("RimeContext", RimeContext, composition);
    field_offset!("RimeContext", RimeContext, menu);
    field_offset!("RimeContext", RimeContext, commit_text_preview);
    field_offset!("RimeContext", RimeContext, select_labels);
    type_layout!("RimeStatus", RimeStatus);
    field_offset!("RimeStatus", RimeStatus, schema_id);
    field_offset!("RimeStatus", RimeStatus, is_ascii_punct);

    println!(
        "RimeApi.required_data_size.free_status={}",
        RIME_API_REQUIRED_DATA_SIZE
    );
    field_offset!("RimeApi", RimeApi, setup);
    field_offset!("RimeApi", RimeApi, initialize);
    field_offset!("RimeApi", RimeApi, create_session);
    field_offset!("RimeApi", RimeApi, process_key);
    field_offset!("RimeApi", RimeApi, get_commit);
    field_offset!("RimeApi", RimeApi, free_commit);
    field_offset!("RimeApi", RimeApi, get_context);
    field_offset!("RimeApi", RimeApi, free_context);
    field_offset!("RimeApi", RimeApi, get_status);
    field_offset!("RimeApi", RimeApi, free_status);
    type_layout!("RimeApi.candidate_extension", RimeApiCandidateExtension);
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        set_option
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        get_option
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        reserved_before_schema
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        select_schema
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        reserved_before_select
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        select_candidate_on_current_page
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        reserved_after_select
    );
    field_offset!(
        "RimeApi.candidate_extension",
        RimeApiCandidateExtension,
        change_page
    );
    println!(
        "RimeApi.candidate_extension.required_data_size={}",
        RIME_API_CANDIDATE_DATA_SIZE
    );
}
