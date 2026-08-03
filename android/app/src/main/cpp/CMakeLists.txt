# CMakeLists.txt for the mdview Android app.
#
# Most of the native code is built by `cargo-ndk` and the resulting `libmdview.so`
# is placed in `jniLibs/<abi>/libmdview.so` by cargo-ndk. This CMakeLists.txt
# builds the prefab-based `android-games-sdk` native glue (libgame-activity.a)
# so the Rust code can hook into the GameActivity lifecycle.

cmake_minimum_required(VERSION 3.22.1)

project(mdview LANGUAGES C CXX)

# Find the prefab-built libraries (android-games-sdk).
find_package(game-activity REQUIRED CONFIG)

# Build an empty shared object so cargo-ndk's libmdview.so is the only
# native library we ship. The gamesdk native glue (libgame-activity.a) is
# statically linked into this empty shared object.
add_library(mdview_empty SHARED empty.c)

target_link_libraries(mdview_empty
    PRIVATE
    game-activity::game-activity
    log
    android
)