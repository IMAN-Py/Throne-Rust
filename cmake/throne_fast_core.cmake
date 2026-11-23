# cmake/throne_fast_core.cmake
# Integration module for Rust acceleration core (throne_fast_core)

if (NOT DEFINED ENABLE_THRONE_FAST_CORE)
    option(ENABLE_THRONE_FAST_CORE "Build and integrate Rust acceleration core" ON)
endif()

if (ENABLE_THRONE_FAST_CORE)
    find_program(CARGO_BIN cargo)
    set(THRONE_FAST_CORE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/throne_fast_core")
    set(THRONE_FAST_TARGET_DIR "${THRONE_FAST_CORE_DIR}/target/release")

    # Platform-specific library naming and byproducts
    if (WIN32)
        set(THRONE_FAST_LIB_NAME "throne_fast_core.dll")
        if (MSVC)
            set(THRONE_FAST_LINK_LIB "${THRONE_FAST_TARGET_DIR}/throne_fast_core.dll.lib")
        else()
            set(THRONE_FAST_LINK_LIB "${THRONE_FAST_TARGET_DIR}/throne_fast_core.dll")
        endif()
        set(THRONE_FAST_BYPRODUCTS
            "${THRONE_FAST_TARGET_DIR}/throne_fast_core.dll"
            "${THRONE_FAST_TARGET_DIR}/throne_fast_core.dll.lib"
        )
    elseif (APPLE)
        set(THRONE_FAST_LIB_NAME "libthrone_fast_core.dylib")
        set(THRONE_FAST_LINK_LIB "${THRONE_FAST_TARGET_DIR}/libthrone_fast_core.dylib")
        set(THRONE_FAST_BYPRODUCTS "${THRONE_FAST_TARGET_DIR}/libthrone_fast_core.dylib")
    else()
        set(THRONE_FAST_LIB_NAME "libthrone_fast_core.so")
        set(THRONE_FAST_LINK_LIB "${THRONE_FAST_TARGET_DIR}/libthrone_fast_core.so")
        set(THRONE_FAST_BYPRODUCTS "${THRONE_FAST_TARGET_DIR}/libthrone_fast_core.so")
    endif()

    if (CARGO_BIN)
        message(STATUS "Found Cargo: ${CARGO_BIN} -- enabling Rust acceleration core")
        if (NOT TARGET build_throne_fast_core)
            add_custom_target(build_throne_fast_core
                COMMAND ${CARGO_BIN} build --release
                WORKING_DIRECTORY ${THRONE_FAST_CORE_DIR}
                BYPRODUCTS ${THRONE_FAST_BYPRODUCTS}
                COMMENT "Building Rust throne_fast_core crate"
                VERBATIM
            )
        endif()
        if (TARGET Throne)
            add_dependencies(Throne build_throne_fast_core)
        endif()
    else()
        message(WARNING "Cargo not found. Using prebuilt ${THRONE_FAST_LIB_NAME} if available.")
    endif()

    if (TARGET Throne)
        target_include_directories(Throne PRIVATE
            "${CMAKE_CURRENT_SOURCE_DIR}/include"
            "${THRONE_FAST_CORE_DIR}/include"
        )
        target_compile_definitions(Throne PRIVATE THRONE_ENABLE_FAST_CORE=1)

        # Link platform-specific Rust acceleration core library
        target_link_libraries(Throne PRIVATE "${THRONE_FAST_LINK_LIB}")

        # Post-build copy shared library to target binary folder on Windows
        if (WIN32)
            add_custom_command(TARGET Throne POST_BUILD
                COMMAND ${CMAKE_COMMAND} -E copy_if_different
                    "${THRONE_FAST_TARGET_DIR}/${THRONE_FAST_LIB_NAME}"
                    "$<TARGET_FILE_DIR:Throne>/${THRONE_FAST_LIB_NAME}"
                COMMENT "Copying ${THRONE_FAST_LIB_NAME} to output folder"
            )
        endif()
    endif()
endif()
