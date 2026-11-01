cmake_minimum_required(VERSION 3.19)
project(LettuceVulkanToolchainCheck LANGUAGES C CXX)

find_package(Vulkan COMPONENTS glslc REQUIRED)
find_package(SPIRV-Headers CONFIG REQUIRED)

add_executable(vulkan-toolchain-check main.cpp)
target_compile_features(vulkan-toolchain-check PRIVATE cxx_std_17)
target_link_libraries(vulkan-toolchain-check PRIVATE Vulkan::Vulkan SPIRV-Headers::SPIRV-Headers)
