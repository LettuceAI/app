#include <vulkan/vulkan.h>
#include <spirv/unified1/spirv.hpp>

int main() {
    const auto create_instance = vkGetInstanceProcAddr(VK_NULL_HANDLE, "vkCreateInstance");
    return create_instance != nullptr && spv::OpNop == 0 ? 0 : 1;
}
