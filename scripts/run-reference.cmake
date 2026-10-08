cmake_minimum_required(VERSION 3.20)

foreach(required ORACLE RUN_DIR CONFIG INPUT TICKS)
    if(NOT DEFINED ${required})
        message(FATAL_ERROR "Missing ${required}")
    endif()
endforeach()

file(MAKE_DIRECTORY "${RUN_DIR}")
file(COPY_FILE "${CONFIG}" "${RUN_DIR}/openttd.cfg")
if(EXISTS "${RUN_DIR}/save/autosave/exit.sav")
    message(FATAL_ERROR "Run directory already contains an exit save: ${RUN_DIR}")
endif()

if(INPUT STREQUAL "GENERATE")
    set(game_args -g -G 12345)
else()
    if(NOT EXISTS "${INPUT}")
        message(FATAL_ERROR "Missing input save: ${INPUT}")
    endif()
    set(game_args -g "${INPUT}")
endif()

execute_process(
    COMMAND "${ORACLE}" -X -x -c "${RUN_DIR}/openttd.cfg"
        "-vnull:ticks=${TICKS}" -snull -mnull ${game_args} -d sl=2
    WORKING_DIRECTORY "${RUN_DIR}"
    OUTPUT_FILE "${RUN_DIR}/stdout.log"
    ERROR_FILE "${RUN_DIR}/stderr.log"
    RESULT_VARIABLE result
    TIMEOUT 60
)
if(NOT result STREQUAL "0")
    message(FATAL_ERROR "Reference engine failed (${result}); see ${RUN_DIR}/stderr.log")
endif()
file(READ "${RUN_DIR}/stderr.log" engine_log)
if(engine_log MATCHES "\\[sl:0\\]")
    message(FATAL_ERROR "Reference engine reported a save/load error; see ${RUN_DIR}/stderr.log")
endif()
string(REGEX MATCHALL "Loading savegame version [0-9]+" loaded_versions "${engine_log}")
list(LENGTH loaded_versions load_count)
if(INPUT STREQUAL "GENERATE")
    set(expected_load_count 1)
else()
    set(expected_load_count 2)
endif()
if(NOT load_count EQUAL expected_load_count)
    message(FATAL_ERROR "Reference engine loaded an unexpected number of games (${load_count}); possible fallback")
endif()
if(NOT EXISTS "${RUN_DIR}/save/autosave/exit.sav")
    message(FATAL_ERROR "Reference engine did not save a game; see ${RUN_DIR}/stderr.log")
endif()
