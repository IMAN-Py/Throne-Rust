option(ENABLE_THRONE_FAST_CORE "Build and link native Rust acceleration core (throne_fast_core)" ON)

if(ENABLE_THRONE_FAST_CORE)
    find_program(CARGO_EXECUTABLE
        NAMES cargo cargo.exe
        HINTS "$ENV{CARGO_HOME}/bin" "$ENV{USERPROFILE}/.cargo/bin" "$ENV{HOME}/.cargo/bin"
    )
    if(NOT CARGO_EXECUTABLE)
        message(FATAL_ERROR "Cargo executable not found! Install Rust/Cargo or disable ENABLE_THRONE_FAST_CORE.")
    endif()

    set(THRONE_FAST_CRATE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/throne_fast_core")
    set(THRONE_FAST_TARGET_DIR "${THRONE_FAST_CRATE_DIR}/target")

    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        set(_RUST_PROFILE "debug")
        set(_RUST_FLAG "")
    else()
        set(_RUST_PROFILE "release")
        set(_RUST_FLAG "--release")
    endif()

    if(WIN32)
        set(THRONE_FAST_DLL "${THRONE_FAST_TARGET_DIR}/${_RUST_PROFILE}/throne_fast_core.dll")
        if(MSVC)
            set(THRONE_FAST_IMPLIB "${THRONE_FAST_TARGET_DIR}/${_RUST_PROFILE}/throne_fast_core.dll.lib")
        else()
            set(THRONE_FAST_IMPLIB "${THRONE_FAST_TARGET_DIR}/${_RUST_PROFILE}/libthrone_fast_core.dll.a")
        endif()
        set(_RUST_BYPRODUCTS "${THRONE_FAST_DLL}" "${THRONE_FAST_IMPLIB}")
    elseif(APPLE)
        set(THRONE_FAST_DLL "${THRONE_FAST_TARGET_DIR}/${_RUST_PROFILE}/libthrone_fast_core.dylib")
        set(THRONE_FAST_IMPLIB "${THRONE_FAST_DLL}")
        set(_RUST_BYPRODUCTS "${THRONE_FAST_DLL}")
    else()
        set(THRONE_FAST_DLL "${THRONE_FAST_TARGET_DIR}/${_RUST_PROFILE}/libthrone_fast_core.so")
        set(THRONE_FAST_IMPLIB "${THRONE_FAST_DLL}")
        set(_RUST_BYPRODUCTS "${THRONE_FAST_DLL}")
    endif()

    file(GLOB_RECURSE THRONE_FAST_RUST_SOURCES CONFIGURE_DEPENDS
        "${THRONE_FAST_CRATE_DIR}/Cargo.toml"
        "${THRONE_FAST_CRATE_DIR}/src/*.rs"
    )

    add_custom_command(
        OUTPUT ${_RUST_BYPRODUCTS}
        COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${THRONE_FAST_TARGET_DIR}
            ${CARGO_EXECUTABLE} build ${_RUST_FLAG} --manifest-path "${THRONE_FAST_CRATE_DIR}/Cargo.toml"
        DEPENDS ${THRONE_FAST_RUST_SOURCES}
        WORKING_DIRECTORY "${THRONE_FAST_CRATE_DIR}"
        COMMENT "Building Rust acceleration core (throne_fast_core) [${_RUST_PROFILE}]"
        VERBATIM
    )

    add_custom_target(throne_fast_core_build DEPENDS ${_RUST_BYPRODUCTS})

    add_library(throne_fast_core SHARED IMPORTED GLOBAL)
    if(WIN32)
        set_target_properties(throne_fast_core PROPERTIES
            IMPORTED_LOCATION "${THRONE_FAST_DLL}"
            IMPORTED_IMPLIB "${THRONE_FAST_IMPLIB}"
            INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
        )
    else()
        set_target_properties(throne_fast_core PROPERTIES
            IMPORTED_LOCATION "${THRONE_FAST_DLL}"
            INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
        )
    endif()
    add_dependencies(throne_fast_core throne_fast_core_build)

    function(throne_link_fast_core TARGET_NAME)
        target_link_libraries(${TARGET_NAME} PRIVATE throne_fast_core)
        target_include_directories(${TARGET_NAME} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include")
        target_compile_definitions(${TARGET_NAME} PRIVATE THRONE_HAS_FAST_CORE=1)
        add_dependencies(${TARGET_NAME} throne_fast_core_build)
        if(WIN32)
            add_custom_command(TARGET ${TARGET_NAME} POST_BUILD
                COMMAND ${CMAKE_COMMAND} -E copy_if_different
                    "${THRONE_FAST_DLL}"
                    "$<TARGET_FILE_DIR:${TARGET_NAME}>/throne_fast_core.dll"
                COMMENT "Staging throne_fast_core.dll beside ${TARGET_NAME}"
            )
        endif()
    endfunction()
endif()
