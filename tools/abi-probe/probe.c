/*
 * Compile this file against the exact upstream rime_api.h selected in
 * native/librime/UPSTREAM.toml.  It intentionally does not link librime.
 */
#include <stddef.h>
#include <stdio.h>

#include <rime_api.h>

#define MO_ALIGNOF(type) _Alignof(type)

#define PRINT_TYPE(name, type)                                                \
  do {                                                                         \
    printf(name ".size=%zu\n", sizeof(type));                                 \
    printf(name ".align=%zu\n", MO_ALIGNOF(type));                            \
  } while (0)

#define PRINT_OFFSET(name, type, field)                                       \
  printf(name ".offset." #field "=%zu\n", offsetof(type, field))

int main(void) {
  printf("abi.pointer_bits=%zu\n", sizeof(void*) * 8U);
  printf("abi.bool_size=%zu\n", sizeof(Bool));
  printf("abi.session_size=%zu\n", sizeof(RimeSessionId));

  PRINT_TYPE("RimeTraits", RimeTraits);
  PRINT_OFFSET("RimeTraits", RimeTraits, data_size);
  PRINT_OFFSET("RimeTraits", RimeTraits, shared_data_dir);
  PRINT_OFFSET("RimeTraits", RimeTraits, modules);
  PRINT_OFFSET("RimeTraits", RimeTraits, min_log_level);
  PRINT_OFFSET("RimeTraits", RimeTraits, staging_dir);

  PRINT_TYPE("RimeComposition", RimeComposition);
  PRINT_OFFSET("RimeComposition", RimeComposition, preedit);
  PRINT_TYPE("RimeCandidate", RimeCandidate);
  PRINT_OFFSET("RimeCandidate", RimeCandidate, reserved);
  PRINT_TYPE("RimeMenu", RimeMenu);
  PRINT_OFFSET("RimeMenu", RimeMenu, candidates);
  PRINT_OFFSET("RimeMenu", RimeMenu, select_keys);
  PRINT_TYPE("RimeCommit", RimeCommit);
  PRINT_OFFSET("RimeCommit", RimeCommit, text);
  PRINT_TYPE("RimeContext", RimeContext);
  PRINT_OFFSET("RimeContext", RimeContext, composition);
  PRINT_OFFSET("RimeContext", RimeContext, menu);
  PRINT_OFFSET("RimeContext", RimeContext, commit_text_preview);
  PRINT_OFFSET("RimeContext", RimeContext, select_labels);
  PRINT_TYPE("RimeStatus", RimeStatus);
  PRINT_OFFSET("RimeStatus", RimeStatus, schema_id);
  PRINT_OFFSET("RimeStatus", RimeStatus, is_ascii_punct);

  printf("RimeApi.required_data_size.free_status=%zu\n",
         offsetof(RimeApi, free_status) + sizeof(((RimeApi*)0)->free_status) -
             sizeof(((RimeApi*)0)->data_size));
  PRINT_OFFSET("RimeApi", RimeApi, setup);
  PRINT_OFFSET("RimeApi", RimeApi, initialize);
  PRINT_OFFSET("RimeApi", RimeApi, create_session);
  PRINT_OFFSET("RimeApi", RimeApi, process_key);
  PRINT_OFFSET("RimeApi", RimeApi, get_commit);
  PRINT_OFFSET("RimeApi", RimeApi, free_commit);
  PRINT_OFFSET("RimeApi", RimeApi, get_context);
  PRINT_OFFSET("RimeApi", RimeApi, free_context);
  PRINT_OFFSET("RimeApi", RimeApi, get_status);
  PRINT_OFFSET("RimeApi", RimeApi, free_status);
  return 0;
}
