//! Easy to use, high performance memory manager for Vulkan.

use bitflags::bitflags;

use std::mem;

pub mod ffi;
use ash::prelude::VkResult;
use ash::vk;

/// Main allocator object
#[repr(transparent)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash)]
pub struct Allocator(ffi::VmaAllocator);

// Allocator is internally thread safe unless AllocatorCreateFlags::EXTERNALLY_SYNCHRONIZED is used (then you need to add synchronization!)
unsafe impl Send for Allocator {}
unsafe impl Sync for Allocator {}

/// Represents custom memory pool handle.
///
/// Fill structure `AllocatorPoolCreateInfo` and call `Allocator::create_pool` to create it.
/// Call `Allocator::destroy_pool` to destroy it.
#[repr(transparent)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash)]
pub struct AllocatorPool(ffi::VmaPool);

unsafe impl Send for AllocatorPool {}
unsafe impl Sync for AllocatorPool {}

/// Represents single memory allocation.
///
/// It may be either dedicated block of `ash::vk::DeviceMemory` or a specific region of a
/// bigger block of this type plus unique offset.
///
/// Although the library provides convenience functions that create a Vulkan buffer or image,
/// allocate memory for it and bind them together, binding of the allocation to a buffer or an
/// image is out of scope of the allocation itself.
///
/// Allocation object can exist without buffer/image bound, binding can be done manually by
/// the user, and destruction of it can be done independently of destruction of the allocation.
///
/// The object also remembers its size and some other information. To retrieve this information,
/// use `Allocator::get_allocation_info`.
///
/// Some kinds allocations can be in lost state.
#[repr(transparent)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash)]
pub struct Allocation(ffi::VmaAllocation);

unsafe impl Send for Allocation {}
unsafe impl Sync for Allocation {}

/// Parameters of `Allocation` objects, that can be retrieved using `Allocator::get_allocation_info`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct AllocationInfo(ffi::VmaAllocationInfo);

unsafe impl Send for AllocationInfo {}
unsafe impl Sync for AllocationInfo {}

impl AllocationInfo {
    #[inline(always)]
    // Gets the memory type index that this allocation was allocated from. (Never changes)
    pub fn memory_type(&self) -> u32 {
        self.0.memoryType
    }

    /// Handle to Vulkan memory object.
    ///
    /// Same memory object can be shared by multiple allocations.
    ///
    /// It can change after call to `Allocator::defragment` if this allocation is passed
    /// to the function, or if allocation is lost.
    ///
    /// If the allocation is lost, it is equal to `ash::vk::DeviceMemory::null()`.
    #[inline(always)]
    pub fn device_memory(&self) -> ash::vk::DeviceMemory {
        self.0.deviceMemory
    }

    /// Offset into device memory object to the beginning of this allocation, in bytes.
    /// (`self.get_device_memory()`, `self.get_offset()`) pair is unique to this allocation.
    ///
    /// It can change after call to `Allocator::defragment` if this allocation is passed
    /// to the function, or if allocation is lost.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.0.offset as usize
    }

    /// Size of this allocation, in bytes.
    ///
    /// It never changes, unless allocation is lost.
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.0.size as usize
    }

    /// Pointer to the beginning of this allocation as mapped data.
    ///
    /// If the allocation hasn't been mapped using `Allocator::map_memory` and hasn't been
    /// created with `AllocationCreateFlags::MAPPED` flag, this value is null.
    ///
    /// It can change after call to `Allocator::map_memory`, `Allocator::unmap_memory`.
    /// It can also change after call to `Allocator::defragment` if this allocation is
    /// passed to the function.
    #[inline(always)]
    pub fn mapped_data(&self) -> *mut u8 {
        self.0.pMappedData as *mut u8
    }

    /// Custom general-purpose pointer that was passed as `AllocationCreateInfo::user_data` or set using `Allocator::set_allocation_user_data`.
    ///
    /// It can change after a call to `Allocator::set_allocation_user_data` for this allocation.
    #[inline(always)]
    pub fn user_data(&self) -> *mut ::std::os::raw::c_void {
        self.0.pUserData
    }
}

bitflags! {
    /// Flags for configuring `Allocator` construction.
    pub struct AllocatorCreateFlags: u32 {
        /// No allocator configuration other than defaults.
        const NONE = 0x0000_0000;

        /// Allocator and all objects created from it will not be synchronized internally,
        /// so you must guarantee they are used from only one thread at a time or synchronized
        /// externally by you. Using this flag may increase performance because internal
        /// mutexes are not used.
        const EXTERNALLY_SYNCHRONIZED = 0x0000_0001;

        /// Enables usage of `VK_KHR_dedicated_allocation` extension.
        ///
        /// Using this extenion will automatically allocate dedicated blocks of memory for
        /// some buffers and images instead of suballocating place for them out of bigger
        /// memory blocks (as if you explicitly used `AllocationCreateFlags::DEDICATED_MEMORY` flag) when it is
        /// recommended by the driver. It may improve performance on some GPUs.
        ///
        /// You may set this flag only if you found out that following device extensions are
        /// supported, you enabled them while creating Vulkan device passed as
        /// `AllocatorCreateInfo::device`, and you want them to be used internally by this
        /// library:
        ///
        /// - VK_KHR_get_memory_requirements2
        /// - VK_KHR_dedicated_allocation
        ///
        /// When this flag is set, you can experience following warnings reported by Vulkan
        /// validation layer. You can ignore them.
        /// `> vkBindBufferMemory(): Binding memory to buffer 0x2d but vkGetBufferMemoryRequirements() has not been called on that buffer.`
        const KHR_DEDICATED_ALLOCATION = 0x0000_0002;

        /// Enables usage of VK_KHR_bind_memory2 extension.
        ///
        /// The flag works only if VmaAllocatorCreateInfo::vulkanApiVersion `== VK_API_VERSION_1_0`.
        /// When it is `VK_API_VERSION_1_1`, the flag is ignored because the extension has been promoted to Vulkan 1.1.
        ///
        /// You may set this flag only if you found out that this device extension is supported,
        /// you enabled it while creating Vulkan device passed as VmaAllocatorCreateInfo::device,
        /// and you want it to be used internally by this library.
        ///
        /// The extension provides functions `vkBindBufferMemory2KHR` and `vkBindImageMemory2KHR`,
        /// which allow to pass a chain of `pNext` structures while binding.
        /// This flag is required if you use `pNext` parameter in `vmaBindBufferMemory2()` or `vmaBindImageMemory2()`.
        const KHR_BIND_MEMORY2 = 0x00000004;

        /// Enables usage of `VK_EXT_memory_budget` extension.
        ///
        /// You may set this flag only if you found out that this device extension is supported,
        /// you enabled it while creating Vulkan device passed as `VmaAllocatorCreateInfo::device`,
        /// and you want it to be used internally by this library, along with another instance extension
        /// `VK_KHR_get_physical_device_properties2`, which is required by it (or Vulkan 1.1, where this extension is promoted).
        ///
        /// The extension provides query for current memory usage and budget, which will probably
        /// be more accurate than an estimation used by the library otherwise.
        const EXT_MEMORY_BUDGET = 0x00000008;

        /// Enables usage of `VK_AMD_device_coherent_memory` extension.
        ///
        /// You may set this flag only if you:
        /// - Found out that this device extension is supported and enabled it while creating Vulkan
        /// device passed as `VmaAllocatorCreateInfo::device`
        /// - Checked that `VkPhysicalDeviceCoherentMemoryFeaturesAMD::deviceCoherentMemory` is true
        /// and set it while creating the Vulkan device,
        /// - Want it to be used internally by this library.
        ///
        /// The extension and accompanying device feature provide access to memory types with
        /// `VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD` and `VK_MEMORY_PROPERTY_DEVICE_UNCACHED_BIT_AMD` flags.
        /// They are useful mostly for writing breadcrumb markers - a common method for debugging GPU crash/hang/TDR.
        ///
        /// When the extension is not enabled, such memory types are still enumerated, but their usage is illegal.
        /// To protect from this error, if you don't create the allocator with this flag, it will
        /// refuse to allocate any memory or create a custom pool in such memory type,
        /// returning `VK_ERROR_FEATURE_NOT_PRESENT`.
        const AMD_DEVICE_COHERENT_MEMORY = 0x00000010;

        /// Enables usage of "buffer device address" feature, which allows you to use function
        /// `vkGetBufferDeviceAddress*` to get raw GPU pointer to a buffer and pass it for usage inside a shader.
        ///
        /// You may set this flag only if you:
        /// 1. (For Vulkan version < 1.2) Found as available and enabled device extension `VK_KHR_buffer_device_address`.
        /// This extension is promoted to core Vulkan 1.2.
        /// 2. Found as available and enabled device feature `VkPhysicalDeviceBufferDeviceAddressFeatures::bufferDeviceAddress`.
        ///
        /// When this flag is set, you can create buffers with `VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT` using VMA.
        /// The library automatically adds `VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT` to
        /// allocated memory blocks wherever it might be needed.
        ///
        /// For more information, see documentation chapter enabling_buffer_device_address.
        const BUFFER_DEVICE_ADDRESS = 0x00000020;

        /// Enables usage of VK_EXT_memory_priority extension in the library.
        ///
        /// You may set this flag only if you found available and enabled this device extension,
        /// along with `VkPhysicalDeviceMemoryPriorityFeaturesEXT::memoryPriority == VK_TRUE`,
        /// while creating Vulkan device passed as VmaAllocatorCreateInfo::device.
        ///
        /// When this flag is used, `VmaAllocationCreateInfo::priority` and `VmaPoolCreateInfo::priority`
        /// are used to set priorities of allocated Vulkan memory. Without it, these variables are ignored.
        ///
        /// A priority must be a floating-point value between 0 and 1, indicating the priority of the allocation relative to other memory allocations.
        /// Larger values are higher priority. The granularity of the priorities is implementation-dependent.
        /// It is automatically passed to every call to `vkAllocateMemory` done by the library using structure `VkMemoryPriorityAllocateInfoEXT`.
        /// The value to be used for default priority is 0.5.
        /// For more details, see the documentation of the `VK_EXT_memory_priority` extension.
        const EXT_MEMORY_PRIORITY = 0x00000040;
    }
}

/// Construct `AllocatorCreateFlags` with default values
impl Default for AllocatorCreateFlags {
    fn default() -> Self {
        AllocatorCreateFlags::NONE
    }
}

/// Description of an `Allocator` to be created.
pub struct AllocatorCreateInfo<'a> {
    /// Flags for created allocator.
    pub flags: AllocatorCreateFlags,

    /// Vulkan physical device. It must be valid throughout whole lifetime of created allocator.
    pub physical_device: ash::vk::PhysicalDevice,

    /// Vulkan device. It must be valid throughout whole lifetime of created allocator.
    pub device: ash::Device,

    /// Vulkan instance. It must be valid throughout whole lifetime of created allocator.
    pub instance: ash::Instance,

    /// Preferred size of a single `ash::vk::DeviceMemory` block to be allocated from large heaps > 1 GiB.
    /// Set to 0 to use default, which is currently 256 MiB.
    pub preferred_large_heap_block_size: vk::DeviceSize,

    /// Custom CPU memory allocation callbacks.
    pub allocation_callbacks: Option<vk::AllocationCallbacks>,

    /// Maximum number of additional frames that are in use at the same time as current frame.
    ///
    /// This value is used only when you make allocations with `AllocationCreateFlags::CAN_BECOME_LOST` flag.
    ///
    /// Such allocations cannot become lost if:
    /// `allocation.lastUseFrameIndex >= allocator.currentFrameIndex - frameInUseCount`
    ///
    /// For example, if you double-buffer your command buffers, so resources used for
    /// rendering in previous frame may still be in use by the GPU at the moment you
    /// allocate resources needed for the current frame, set this value to 1.
    ///
    /// If you want to allow any allocations other than used in the current frame to
    /// become lost, set this value to 0.
    pub frame_in_use_count: u32,

    /// Either empty or an array of limits on maximum number of bytes that can be allocated
    /// out of particular Vulkan memory heap.
    ///
    /// If not empty, it must contain `ash::vk::PhysicalDeviceMemoryProperties::memory_heap_count` elements,
    /// defining limit on maximum number of bytes that can be allocated out of particular Vulkan
    /// memory heap.
    ///
    /// Any of the elements may be equal to `ash::vk::WHOLE_SIZE`, which means no limit on that
    /// heap. This is also the default in case of an empty slice.
    ///
    /// If there is a limit defined for a heap:
    ///
    /// * If user tries to allocate more memory from that heap using this allocator, the allocation
    /// fails with `ash::vk::Result::ERROR_OUT_OF_DEVICE_MEMORY`.
    ///
    /// * If the limit is smaller than heap size reported in `ash::vk::MemoryHeap::size`, the value of this
    /// limit will be reported instead when using `Allocator::get_memory_properties`.
    ///
    /// Warning! Using this feature may not be equivalent to installing a GPU with smaller amount of
    /// memory, because graphics driver doesn't necessary fail new allocations with
    /// `ash::vk::Result::ERROR_OUT_OF_DEVICE_MEMORY` result when memory capacity is exceeded. It may return success
    /// and just silently migrate some device memory" blocks to system RAM. This driver behavior can
    /// also be controlled using the `VK_AMD_memory_overallocation_behavior` extension.
    pub heap_size_limits: Option<&'a [ash::vk::DeviceSize]>,

    /// The highest version of Vulkan that the application is designed to use.
    /// It must be a value in the format as created by macro `VK_MAKE_VERSION` or a constant like:
    /// `VK_API_VERSION_1_1`, `VK_API_VERSION_1_0`. The patch version number specified is ignored.
    /// Only the major and minor versions are considered. It must be less or equal (preferably equal)
    /// to value as passed to `vkCreateInstance` as `VkApplicationInfo::apiVersion`. Only versions
    /// 1.0, 1.1, 1.2 are supported by the current implementation.
    /// Leaving it initialized to zero is equivalent to `VK_API_VERSION_1_0`.
    pub vulkan_api_version: u32,
}

/// Converts a raw result into an ash result.
#[inline]
fn ffi_to_result(result: vk::Result) -> VkResult<()> {
    match result {
        vk::Result::SUCCESS => Ok(()),
        _ => Err(result),
    }
}

/// Converts an `AllocationCreateInfo` struct into the raw representation.
fn allocation_create_info_to_ffi(info: &AllocationCreateInfo) -> ffi::VmaAllocationCreateInfo {
    ffi::VmaAllocationCreateInfo {
        flags: info.flags.bits(),
        usage: match info.usage {
            MemoryUsage::Unknown => ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_UNKNOWN,
            MemoryUsage::GpuOnly => ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_GPU_ONLY,
            MemoryUsage::CpuOnly => ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_CPU_ONLY,
            MemoryUsage::CpuToGpu => ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_CPU_TO_GPU,
            MemoryUsage::GpuToCpu => ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_GPU_TO_CPU,
            MemoryUsage::CpuCopy => ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_CPU_COPY,
            MemoryUsage::GpuLazilyAllocated => {
                ffi::VmaMemoryUsage_VMA_MEMORY_USAGE_GPU_LAZILY_ALLOCATED
            }
        },
        requiredFlags: info.required_flags,
        preferredFlags: info.preferred_flags,
        memoryTypeBits: info.memory_type_bits,
        pool: match info.pool {
            Some(pool) => pool.0 as _,
            None => unsafe { mem::zeroed() },
        },
        pUserData: info.user_data.unwrap_or(::std::ptr::null_mut()),
        priority: info.priority,
    }
}

/// Converts an `AllocatorPoolCreateInfo` struct into the raw representation.
fn pool_create_info_to_ffi(info: &AllocatorPoolCreateInfo) -> ffi::VmaPoolCreateInfo {
    ffi::VmaPoolCreateInfo {
        memoryTypeIndex: info.memory_type_index,
        flags: info.flags.bits(),
        blockSize: info.block_size as vk::DeviceSize,
        minBlockCount: info.min_block_count,
        maxBlockCount: info.max_block_count,
        frameInUseCount: info.frame_in_use_count,
        priority: info.priority,
        minAllocationAlignment: info.min_allocation_alignment,
        pMemoryAllocateNext: info.memory_allocate_next.unwrap_or(std::ptr::null_mut()),
    }
}

/// Intended usage of memory.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum MemoryUsage {
    /// No intended memory usage specified.
    /// Use other members of `AllocationCreateInfo` to specify your requirements.
    Unknown,

    /// Memory will be used on device only, so fast access from the device is preferred.
    /// It usually means device-local GPU (video) memory.
    /// No need to be mappable on host.
    /// It is roughly equivalent of `D3D12_HEAP_TYPE_DEFAULT`.
    ///
    /// Usage:
    ///
    /// - Resources written and read by device, e.g. images used as attachments.
    /// - Resources transferred from host once (immutable) or infrequently and read by
    ///   device multiple times, e.g. textures to be sampled, vertex buffers, uniform
    ///   (constant) buffers, and majority of other types of resources used on GPU.
    ///
    /// Allocation may still end up in `ash::vk::MemoryPropertyFlags::HOST_VISIBLE` memory on some implementations.
    /// In such case, you are free to map it.
    /// You can use `AllocationCreateFlags::MAPPED` with this usage type.
    GpuOnly,

    /// Memory will be mappable on host.
    /// It usually means CPU (system) memory.
    /// Guarantees to be `ash::vk::MemoryPropertyFlags::HOST_VISIBLE` and `ash::vk::MemoryPropertyFlags::HOST_COHERENT`.
    /// CPU access is typically uncached. Writes may be write-combined.
    /// Resources created in this pool may still be accessible to the device, but access to them can be slow.
    /// It is roughly equivalent of `D3D12_HEAP_TYPE_UPLOAD`.
    ///
    /// Usage: Staging copy of resources used as transfer source.
    CpuOnly,

    /// Memory that is both mappable on host (guarantees to be `ash::vk::MemoryPropertyFlags::HOST_VISIBLE`) and preferably fast to access by GPU.
    /// CPU access is typically uncached. Writes may be write-combined.
    ///
    /// Usage: Resources written frequently by host (dynamic), read by device. E.g. textures, vertex buffers,
    /// uniform buffers updated every frame or every draw call.
    CpuToGpu,

    /// Memory mappable on host (guarantees to be `ash::vk::MemoryPropertFlags::HOST_VISIBLE`) and cached.
    /// It is roughly equivalent of `D3D12_HEAP_TYPE_READBACK`.
    ///
    /// Usage:
    ///
    /// - Resources written by device, read by host - results of some computations, e.g. screen capture, average scene luminance for HDR tone mapping.
    /// - Any resources read or accessed randomly on host, e.g. CPU-side copy of vertex buffer used as source of transfer, but also used for collision detection.
    GpuToCpu,

    /// CPU memory - memory that is preferably not `DEVICE_LOCAL`, but also not guaranteed to be `HOST_VISIBLE`.
    ///
    /// Usage:
    /// - Staging copy of resources moved from GPU memory to CPU memory as part
    /// of custom paging/residency mechanism, to be moved back to GPU memory when needed.
    CpuCopy,

    /// Lazily allocated GPU memory having `VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT`.
    /// Exists mostly on mobile platforms. Using it on desktop PC or other GPUs with no such memory
    /// type present will fail the allocation.
    ///
    /// Usage:
    /// - Memory for transient attachment images (color attachments, depth attachments etc.),
    /// created with `VK_IMAGE_USAGE_TRANSIENT_ATTACHMENT_BIT`.
    ///
    /// Allocations with this usage are always created as dedicated - it implies #VMA_ALLOCATION_CREATE_DEDICATED_MEMORY_BIT.
    GpuLazilyAllocated,
}

bitflags! {
    /// Flags for configuring `AllocatorPool` construction.
    pub struct AllocatorPoolCreateFlags: u32 {
        const NONE = 0x0000_0000;

        /// Use this flag if you always allocate only buffers and linear images or only optimal images
        /// out of this pool and so buffer-image granularity can be ignored.
        ///
        /// This is an optional optimization flag.
        ///
        /// If you always allocate using `Allocator::create_buffer`, `Allocator::create_image`,
        /// `Allocator::allocate_memory_for_buffer`, then you don't need to use it because allocator
        /// knows exact type of your allocations so it can handle buffer-image granularity
        /// in the optimal way.
        ///
        /// If you also allocate using `Allocator::allocate_memory_for_image` or `Allocator::allocate_memory`,
        /// exact type of such allocations is not known, so allocator must be conservative
        /// in handling buffer-image granularity, which can lead to suboptimal allocation
        /// (wasted memory). In that case, if you can make sure you always allocate only
        /// buffers and linear images or only optimal images out of this pool, use this flag
        /// to make allocator disregard buffer-image granularity and so make allocations
        /// faster and more optimal.
        const IGNORE_BUFFER_IMAGE_GRANULARITY = 0x0000_0002;

        /// Enables alternative, linear allocation algorithm in this pool.
        ///
        /// Specify this flag to enable linear allocation algorithm, which always creates
        /// new allocations after last one and doesn't reuse space from allocations freed in
        /// between. It trades memory consumption for simplified algorithm and data
        /// structure, which has better performance and uses less memory for metadata.
        ///
        /// By using this flag, you can achieve behavior of free-at-once, stack,
        /// ring buffer, and double stack.
        ///
        /// When using this flag, you must specify PoolCreateInfo::max_block_count == 1 (or 0 for default).
        const LINEAR_ALGORITHM = 0x0000_0004;

        /// Enables alternative, buddy allocation algorithm in this pool.
        ///
        /// It operates on a tree of blocks, each having size that is a power of two and
        /// a half of its parent's size. Comparing to default algorithm, this one provides
        /// faster allocation and deallocation and decreased external fragmentation,
        /// at the expense of more memory wasted (internal fragmentation).
        const BUDDY_ALGORITHM = 0x0000_0008;

        /// Bit mask to extract only `*_ALGORITHM` bits from entire set of flags.
        const ALGORITHM_MASK = 0x0000_0004 | 0x0000_0008;
    }
}

bitflags! {
    /// Flags for configuring `Allocation` construction.
    pub struct AllocationCreateFlags: u32 {
        /// Default configuration for allocation.
        const NONE = 0x0000_0000;

        /// Set this flag if the allocation should have its own memory block.
        ///
        /// Use it for special, big resources, like fullscreen images used as attachments.
        ///
        /// You should not use this flag if `AllocationCreateInfo::pool` is not `None`.
        const DEDICATED_MEMORY = 0x0000_0001;

        /// Set this flag to only try to allocate from existing `ash::vk::DeviceMemory` blocks and never create new such block.
        ///
        /// If new allocation cannot be placed in any of the existing blocks, allocation
        /// fails with `ash::vk::Result::ERROR_OUT_OF_DEVICE_MEMORY` error.
        ///
        /// You should not use `AllocationCreateFlags::DEDICATED_MEMORY` and `AllocationCreateFlags::NEVER_ALLOCATE` at the same time. It makes no sense.
        ///
        /// If `AllocationCreateInfo::pool` is not `None`, this flag is implied and ignored.
        const NEVER_ALLOCATE = 0x0000_0002;

        /// Set this flag to use a memory that will be persistently mapped and retrieve pointer to it.
        ///
        /// Pointer to mapped memory will be returned through `Allocation::get_mapped_data()`.
        ///
        /// Is it valid to use this flag for allocation made from memory type that is not
        /// `ash::vk::MemoryPropertyFlags::HOST_VISIBLE`. This flag is then ignored and memory is not mapped. This is
        /// useful if you need an allocation that is efficient to use on GPU
        /// (`ash::vk::MemoryPropertyFlags::DEVICE_LOCAL`) and still want to map it directly if possible on platforms that
        /// support it (e.g. Intel GPU).
        ///
        /// You should not use this flag together with `AllocationCreateFlags::CAN_BECOME_LOST`.
        const MAPPED = 0x0000_0004;

        /// Allocation created with this flag can become lost as a result of another
        /// allocation with `AllocationCreateFlags::CAN_MAKE_OTHER_LOST` flag, so you must check it before use.
        ///
        /// To check if allocation is not lost, call `Allocator::get_allocation_info` and check if
        /// `AllocationInfo::device_memory` is not null.
        ///
        /// You should not use this flag together with `AllocationCreateFlags::MAPPED`.
        const CAN_BECOME_LOST = 0x0000_0008;

        /// While creating allocation using this flag, other allocations that were
        /// created with flag `AllocationCreateFlags::CAN_BECOME_LOST` can become lost.
        const CAN_MAKE_OTHER_LOST = 0x0000_0010;

        /// Set this flag to treat `AllocationCreateInfo::user_data` as pointer to a
        /// null-terminated string. Instead of copying pointer value, a local copy of the
        /// string is made and stored in allocation's user data. The string is automatically
        /// freed together with the allocation. It is also used in `Allocator::build_stats_string`.
        const USER_DATA_COPY_STRING = 0x0000_0020;

        /// Allocation will be created from upper stack in a double stack pool.
        ///
        /// This flag is only allowed for custom pools created with `AllocatorPoolCreateFlags::LINEAR_ALGORITHM` flag.
        const UPPER_ADDRESS = 0x0000_0040;

        /// Create both buffer/image and allocation, but don't bind them together.
        /// It is useful when you want to bind yourself to do some more advanced binding, e.g. using some extensions.
        /// The flag is meaningful only with functions that bind by default, such as `Allocator::create_buffer`
        /// or `Allocator::create_image`. Otherwise it is ignored.
        const CREATE_DONT_BIND = 0x0000_0080;

        /// Allocation strategy that chooses smallest possible free range for the
        /// allocation.
        const STRATEGY_BEST_FIT = 0x0001_0000;

        /// Allocation strategy that chooses biggest possible free range for the
        /// allocation.
        const STRATEGY_WORST_FIT = 0x0002_0000;

        /// Allocation strategy that chooses first suitable free range for the
        /// allocation.
        ///
        /// "First" doesn't necessarily means the one with smallest offset in memory,
        /// but rather the one that is easiest and fastest to find.
        const STRATEGY_FIRST_FIT = 0x0004_0000;

        /// Allocation strategy that tries to minimize memory usage.
        const STRATEGY_MIN_MEMORY = 0x0001_0000;

        /// Allocation strategy that tries to minimize allocation time.
        const STRATEGY_MIN_TIME = 0x0004_0000;

        /// Allocation strategy that tries to minimize memory fragmentation.
        const STRATEGY_MIN_FRAGMENTATION = 0x0002_0000;

        /// A bit mask to extract only `*_STRATEGY` bits from entire set of flags.
        const STRATEGY_MASK = 0x0001_0000 | 0x0002_0000 | 0x0004_0000;
    }
}

/// Description of an `Allocation` to be created.
#[derive(Debug, Clone)]
pub struct AllocationCreateInfo {
    /// Flags for configuring the allocation
    pub flags: AllocationCreateFlags,

    /// Intended usage of memory.
    ///
    /// You can leave `MemoryUsage::UNKNOWN` if you specify memory requirements
    /// in another way.
    ///
    /// If `pool` is not `None`, this member is ignored.
    pub usage: MemoryUsage,

    /// Flags that must be set in a Memory Type chosen for an allocation.
    ///
    /// Leave 0 if you specify memory requirements in other way.
    ///
    /// If `pool` is not `None`, this member is ignored.
    pub required_flags: ash::vk::MemoryPropertyFlags,

    /// Flags that preferably should be set in a memory type chosen for an allocation.
    ///
    /// Set to 0 if no additional flags are prefered.
    ///
    /// If `pool` is not `None`, this member is ignored.
    pub preferred_flags: ash::vk::MemoryPropertyFlags,

    /// Bit mask containing one bit set for every memory type acceptable for this allocation.
    ///
    /// Value 0 is equivalent to `std::u32::MAX` - it means any memory type is accepted if
    /// it meets other requirements specified by this structure, with no further restrictions
    /// on memory type index.
    ///
    /// If `pool` is not `None`, this member is ignored.
    pub memory_type_bits: u32,

    /// Pool that this allocation should be created in.
    ///
    /// Specify `None` to allocate from default pool. If not `None`, members:
    /// `usage`, `required_flags`, `preferred_flags`, `memory_type_bits` are ignored.
    pub pool: Option<AllocatorPool>,

    /// Custom general-purpose pointer that will be stored in `Allocation`, can be read
    /// as `Allocation::get_user_data()` and changed using `Allocator::set_allocation_user_data`.
    ///
    /// If `AllocationCreateFlags::USER_DATA_COPY_STRING` is used, it must be either null or pointer to a
    /// null-terminated string. The string will be then copied to internal buffer, so it
    /// doesn't need to be valid after allocation call.
    pub user_data: Option<*mut ::std::os::raw::c_void>,

    /// A floating-point value between 0 and 1, indicating the priority of the allocation relative
    /// to other memory allocations.
    ///
    /// It is used only when #VMA_ALLOCATOR_CREATE_EXT_MEMORY_PRIORITY_BIT flag was used during creation of the #VmaAllocator object
    /// and this allocation ends up as dedicated or is explicitly forced as dedicated using #VMA_ALLOCATION_CREATE_DEDICATED_MEMORY_BIT.
    /// Otherwise, it has the priority of a memory block where it is placed and this variable is ignored.
    pub priority: f32,
}

/// Construct `AllocationCreateInfo` with default values
impl Default for AllocationCreateInfo {
    fn default() -> Self {
        AllocationCreateInfo {
            usage: MemoryUsage::Unknown,
            flags: AllocationCreateFlags::NONE,
            required_flags: ash::vk::MemoryPropertyFlags::empty(),
            preferred_flags: ash::vk::MemoryPropertyFlags::empty(),
            memory_type_bits: 0,
            pool: None,
            user_data: None,
            priority: 0.0,
        }
    }
}

/// Description of an `AllocationPool` to be created.
#[derive(Debug, Clone)]
pub struct AllocatorPoolCreateInfo {
    /// Vulkan memory type index to allocate this pool from.
    pub memory_type_index: u32,

    /// Use combination of `AllocatorPoolCreateFlags`
    pub flags: AllocatorPoolCreateFlags,

    /// Size of a single `ash::vk::DeviceMemory` block to be allocated as part of this
    /// pool, in bytes.
    ///
    /// Specify non-zero to set explicit, constant size of memory blocks used by
    /// this pool.
    ///
    /// Leave 0 to use default and let the library manage block sizes automatically.
    /// Sizes of particular blocks may vary.
    pub block_size: usize,

    /// Minimum number of blocks to be always allocated in this pool, even if they stay empty.
    ///
    /// Set to 0 to have no preallocated blocks and allow the pool be completely empty.
    pub min_block_count: usize,

    /// Maximum number of blocks that can be allocated in this pool.
    ///
    /// Set to 0 to use default, which is no limit.
    ///
    /// Set to same value as `AllocatorPoolCreateInfo::min_block_count` to have fixed amount
    /// of memory allocated throughout whole lifetime of this pool.
    pub max_block_count: usize,

    /// Maximum number of additional frames that are in use at the same time as current frame.
    /// This value is used only when you make allocations with `AllocationCreateFlags::CAN_BECOME_LOST` flag.
    /// Such allocations cannot become lost if:
    ///   `allocation.lastUseFrameIndex >= allocator.currentFrameIndex - frameInUseCount`.
    ///
    /// For example, if you double-buffer your command buffers, so resources used for rendering
    /// in previous frame may still be in use by the GPU at the moment you allocate resources
    /// needed for the current frame, set this value to 1.
    ///
    /// If you want to allow any allocations other than used in the current frame to become lost,
    /// set this value to 0.
    pub frame_in_use_count: u32,

    /// A floating-point value between 0 and 1, indicating the priority of the allocations in this
    /// pool relative to other memory allocations.
    ///
    /// It is used only when #VMA_ALLOCATOR_CREATE_EXT_MEMORY_PRIORITY_BIT flag was used during creation of the #VmaAllocator object.
    /// Otherwise, this variable is ignored.
    pub priority: f32,

    /// Additional minimum alignment to be used for all allocations created from this pool. Can be 0.
    ///
    /// Leave 0 (default) not to impose any additional alignment. If not 0, it must be a power of two.
    /// It can be useful in cases where alignment returned by Vulkan by functions like `vkGetBufferMemoryRequirements` is not enough,
    /// e.g. when doing interop with OpenGL.
    pub min_allocation_alignment: vk::DeviceSize,

    /// Additional `pNext` chain to be attached to `VkMemoryAllocateInfo` used for every allocation made by this pool. Optional.
    ///
    /// Optional, can be null. If not null, it must point to a `pNext` chain of structures that can be attached to `VkMemoryAllocateInfo`.
    /// It can be useful for special needs such as adding `VkExportMemoryAllocateInfoKHR`.
    /// Structures pointed by this member must remain alive and unchanged for the whole lifetime of the custom pool.
    ///
    /// Please note that some structures, e.g. `VkMemoryPriorityAllocateInfoEXT`, `VkMemoryDedicatedAllocateInfoKHR`,
    /// can be attached automatically by this library when using other, more convenient of its features.
    pub memory_allocate_next: Option<*mut ::std::os::raw::c_void>,
}

/// Construct `AllocatorPoolCreateInfo` with default values
impl Default for AllocatorPoolCreateInfo {
    fn default() -> Self {
        AllocatorPoolCreateInfo {
            memory_type_index: 0,
            flags: AllocatorPoolCreateFlags::NONE,
            block_size: 0,
            min_block_count: 0,
            max_block_count: 0,
            frame_in_use_count: 0,
            priority: 0.0,
            min_allocation_alignment: 0,
            memory_allocate_next: None,
        }
    }
}

#[derive(Debug)]
pub struct DefragmentationContext {
    pub(crate) internal: ffi::VmaDefragmentationContext,
    pub(crate) stats: ffi::VmaDefragmentationStats,
    pub(crate) changed: Vec<ash::vk::Bool32>,
}

/// Optional configuration parameters to be passed to `Allocator::defragment`
///
/// DEPRECATED.
#[derive(Debug, Copy, Clone)]
pub struct DefragmentationInfo {
    /// Maximum total numbers of bytes that can be copied while moving
    /// allocations to different places.
    ///
    /// Default is `ash::vk::WHOLE_SIZE`, which means no limit.
    pub max_bytes_to_move: usize,

    /// Maximum number of allocations that can be moved to different place.
    ///
    /// Default is `std::u32::MAX`, which means no limit.
    pub max_allocations_to_move: u32,
}

/// Construct `DefragmentationInfo` with default values
impl Default for DefragmentationInfo {
    fn default() -> Self {
        DefragmentationInfo {
            max_bytes_to_move: ash::vk::WHOLE_SIZE as usize,
            max_allocations_to_move: std::u32::MAX,
        }
    }
}

/// Parameters for defragmentation.
///
/// To be used with function `Allocator::defragmentation_begin`.
#[derive(Debug, Clone)]
pub struct DefragmentationInfo2<'a> {
    /// Collection of allocations that can be defragmented.
    ///
    /// Elements in the slice should be unique - same allocation cannot occur twice.
    /// It is safe to pass allocations that are in the lost state - they are ignored.
    /// All allocations not present in this slice are considered non-moveable during this defragmentation.
    pub allocations: &'a [Allocation],

    /// Either `None` or a slice of pools to be defragmented.
    ///
    /// All the allocations in the specified pools can be moved during defragmentation
    /// and there is no way to check if they were really moved as in `allocations_changed`,
    /// so you must query all the allocations in all these pools for new `ash::vk::DeviceMemory`
    /// and offset using `Allocator::get_allocation_info` if you might need to recreate buffers
    /// and images bound to them.
    ///
    /// Elements in the array should be unique - same pool cannot occur twice.
    ///
    /// Using this array is equivalent to specifying all allocations from the pools in `allocations`.
    /// It might be more efficient.
    pub pools: Option<&'a [AllocatorPool]>,

    /// Maximum total numbers of bytes that can be copied while moving allocations to different places using transfers on CPU side, like `memcpy()`, `memmove()`.
    ///
    /// `ash::vk::WHOLE_SIZE` means no limit.
    pub max_cpu_bytes_to_move: ash::vk::DeviceSize,

    /// Maximum number of allocations that can be moved to a different place using transfers on CPU side, like `memcpy()`, `memmove()`.
    ///
    /// `std::u32::MAX` means no limit.
    pub max_cpu_allocations_to_move: u32,

    /// Maximum total numbers of bytes that can be copied while moving allocations to different places using transfers on GPU side, posted to `command_buffer`.
    ///
    /// `ash::vk::WHOLE_SIZE` means no limit.
    pub max_gpu_bytes_to_move: ash::vk::DeviceSize,

    /// Maximum number of allocations that can be moved to a different place using transfers on GPU side, posted to `command_buffer`.
    ///
    /// `std::u32::MAX` means no limit.
    pub max_gpu_allocations_to_move: u32,

    /// Command buffer where GPU copy commands will be posted.
    ///
    /// If not `None`, it must be a valid command buffer handle that supports transfer queue type.
    /// It must be in the recording state and outside of a render pass instance.
    /// You need to submit it and make sure it finished execution before calling `Allocator::defragmentation_end`.
    ///
    /// Passing `None` means that only CPU defragmentation will be performed.
    pub command_buffer: Option<ash::vk::CommandBuffer>,
}

/// Statistics returned by `Allocator::defragment`
#[derive(Debug, Copy, Clone)]
pub struct DefragmentationStats {
    /// Total number of bytes that have been copied while moving allocations to different places.
    pub bytes_moved: usize,

    /// Total number of bytes that have been released to the system by freeing empty `ash::vk::DeviceMemory` objects.
    pub bytes_freed: usize,

    /// Number of allocations that have been moved to different places.
    pub allocations_moved: u32,

    /// Number of empty `ash::vk::DeviceMemory` objects that have been released to the system.
    pub device_memory_blocks_freed: u32,
}

impl Allocator {
    /// Constructor a new `Allocator` using the provided options.
    pub unsafe fn new(create_info: &AllocatorCreateInfo) -> VkResult<Self> {
        let instance = create_info.instance.clone();
        let device = create_info.device.clone();

        let routed_functions = ffi::VmaVulkanFunctions {
            vkGetPhysicalDeviceProperties: instance.fp_v1_0().get_physical_device_properties,
            vkGetPhysicalDeviceMemoryProperties: instance
                .fp_v1_0()
                .get_physical_device_memory_properties,
            vkAllocateMemory: device.fp_v1_0().allocate_memory,
            vkFreeMemory: device.fp_v1_0().free_memory,
            vkMapMemory: device.fp_v1_0().map_memory,
            vkUnmapMemory: device.fp_v1_0().unmap_memory,
            vkFlushMappedMemoryRanges: device.fp_v1_0().flush_mapped_memory_ranges,
            vkInvalidateMappedMemoryRanges: device.fp_v1_0().invalidate_mapped_memory_ranges,
            vkBindBufferMemory: device.fp_v1_0().bind_buffer_memory,
            vkBindImageMemory: device.fp_v1_0().bind_image_memory,
            vkGetBufferMemoryRequirements: device.fp_v1_0().get_buffer_memory_requirements,
            vkGetImageMemoryRequirements: device.fp_v1_0().get_image_memory_requirements,
            vkCreateBuffer: device.fp_v1_0().create_buffer,
            vkDestroyBuffer: device.fp_v1_0().destroy_buffer,
            vkCreateImage: device.fp_v1_0().create_image,
            vkDestroyImage: device.fp_v1_0().destroy_image,
            vkCmdCopyBuffer: device.fp_v1_0().cmd_copy_buffer,
            vkGetBufferMemoryRequirements2KHR: device.fp_v1_1().get_buffer_memory_requirements2,
            vkGetImageMemoryRequirements2KHR: device.fp_v1_1().get_image_memory_requirements2,
            vkBindBufferMemory2KHR: device.fp_v1_1().bind_buffer_memory2,
            vkBindImageMemory2KHR: device.fp_v1_1().bind_image_memory2,
            vkGetPhysicalDeviceMemoryProperties2KHR: instance
                .fp_v1_1()
                .get_physical_device_memory_properties2,
        };

        let allocation_callbacks = match create_info.allocation_callbacks {
            None => std::ptr::null(),
            Some(ref cb) => cb as *const _,
        };

        let ffi_create_info = ffi::VmaAllocatorCreateInfo {
            physicalDevice: create_info.physical_device,
            device: create_info.device.handle(),
            instance: instance.handle(),
            flags: create_info.flags.bits(),
            frameInUseCount: create_info.frame_in_use_count,
            preferredLargeHeapBlockSize: create_info.preferred_large_heap_block_size as u64,
            pHeapSizeLimit: match &create_info.heap_size_limits {
                None => ::std::ptr::null(),
                Some(limits) => limits.as_ptr(),
            },
            pVulkanFunctions: &routed_functions,
            pAllocationCallbacks: allocation_callbacks,
            pDeviceMemoryCallbacks: ::std::ptr::null(), // TODO: Add support
            pRecordSettings: ::std::ptr::null(),        // TODO: Add support
            vulkanApiVersion: create_info.vulkan_api_version,
            pTypeExternalMemoryHandleTypes: std::ptr::null(),
        };

        let mut handle: ffi::VmaAllocator = mem::zeroed();
        ffi_to_result(ffi::vmaCreateAllocator(
            &ffi_create_info as *const ffi::VmaAllocatorCreateInfo,
            &mut handle,
        ))?;

        Ok(Allocator(handle))
    }

    /// The allocator fetches `ash::vk::PhysicalDeviceProperties` from the physical device.
    /// You can get it here, without fetching it again on your own.
    pub unsafe fn get_physical_device_properties(&self) -> VkResult<vk::PhysicalDeviceProperties> {
        let mut properties = vk::PhysicalDeviceProperties::default();
        ffi::vmaGetPhysicalDeviceProperties(self.0, &mut properties as *mut _ as *mut *const _);

        Ok(properties)
    }

    /// The allocator fetches `ash::vk::PhysicalDeviceMemoryProperties` from the physical device.
    /// You can get it here, without fetching it again on your own.
    pub unsafe fn get_memory_properties(&self) -> VkResult<vk::PhysicalDeviceMemoryProperties> {
        let mut properties = vk::PhysicalDeviceMemoryProperties::default();
        ffi::vmaGetMemoryProperties(self.0, &mut properties as *mut _ as *mut *const _);

        Ok(properties)
    }

    /// Given a memory type index, returns `ash::vk::MemoryPropertyFlags` of this memory type.
    ///
    /// This is just a convenience function; the same information can be obtained using
    /// `Allocator::get_memory_properties`.
    pub unsafe fn get_memory_type_properties(
        &self,
        memory_type_index: u32,
    ) -> VkResult<vk::MemoryPropertyFlags> {
        let mut flags = vk::MemoryPropertyFlags::empty();
        ffi::vmaGetMemoryTypeProperties(self.0, memory_type_index, &mut flags);

        Ok(flags)
    }

    /// Sets index of the current frame.
    ///
    /// This function must be used if you make allocations with `AllocationCreateFlags::CAN_BECOME_LOST` and
    /// `AllocationCreateFlags::CAN_MAKE_OTHER_LOST` flags to inform the allocator when a new frame begins.
    /// Allocations queried using `Allocator::get_allocation_info` cannot become lost
    /// in the current frame.
    pub unsafe fn set_current_frame_index(&self, frame_index: u32) {
        ffi::vmaSetCurrentFrameIndex(self.0, frame_index);
    }

    /// Retrieves statistics from current state of the `Allocator`.
    pub unsafe fn calculate_stats(&self) -> VkResult<ffi::VmaStats> {
        let mut vma_stats: ffi::VmaStats = mem::zeroed();
        ffi::vmaCalculateStats(self.0, &mut vma_stats);
        Ok(vma_stats)
    }

    /// Builds and returns statistics in `JSON` format.
    pub unsafe fn build_stats_string(&self, detailed_map: bool) -> VkResult<String> {
        let mut stats_string: *mut ::std::os::raw::c_char = ::std::ptr::null_mut();
        ffi::vmaBuildStatsString(self.0, &mut stats_string, if detailed_map { 1 } else { 0 });

        Ok(if stats_string.is_null() {
            String::new()
        } else {
            let result = std::ffi::CStr::from_ptr(stats_string)
                .to_string_lossy()
                .into_owned();
            ffi::vmaFreeStatsString(self.0, stats_string);
            result
        })
    }

    /// Helps to find memory type index, given memory type bits and allocation info.
    ///
    /// This algorithm tries to find a memory type that:
    ///
    /// - Is allowed by memory type bits.
    /// - Contains all the flags from `allocation_info.required_flags`.
    /// - Matches intended usage.
    /// - Has as many flags from `allocation_info.preferred_flags` as possible.
    ///
    /// Returns ash::vk::Result::ERROR_FEATURE_NOT_PRESENT if not found. Receiving such a result
    /// from this function or any other allocating function probably means that your
    /// device doesn't support any memory type with requested features for the specific
    /// type of resource you want to use it for. Please check parameters of your
    /// resource, like image layout (OPTIMAL versus LINEAR) or mip level count.
    pub unsafe fn find_memory_type_index(
        &self,
        memory_type_bits: u32,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<u32> {
        let create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut memory_type_index: u32 = 0;
        ffi_to_result(ffi::vmaFindMemoryTypeIndex(
            self.0,
            memory_type_bits,
            &create_info,
            &mut memory_type_index,
        ))?;

        Ok(memory_type_index)
    }

    /// Helps to find memory type index, given buffer info and allocation info.
    ///
    /// It can be useful e.g. to determine value to be used as `AllocatorPoolCreateInfo::memory_type_index`.
    /// It internally creates a temporary, dummy buffer that never has memory bound.
    /// It is just a convenience function, equivalent to calling:
    ///
    /// - `ash::vk::Device::create_buffer`
    /// - `ash::vk::Device::get_buffer_memory_requirements`
    /// - `Allocator::find_memory_type_index`
    /// - `ash::vk::Device::destroy_buffer`
    pub unsafe fn find_memory_type_index_for_buffer_info(
        &self,
        buffer_info: &ash::vk::BufferCreateInfo,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<u32> {
        let allocation_create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut memory_type_index: u32 = 0;
        ffi_to_result(ffi::vmaFindMemoryTypeIndexForBufferInfo(
            self.0,
            buffer_info,
            &allocation_create_info,
            &mut memory_type_index,
        ))?;

        Ok(memory_type_index)
    }

    /// Helps to find memory type index, given image info and allocation info.
    ///
    /// It can be useful e.g. to determine value to be used as `AllocatorPoolCreateInfo::memory_type_index`.
    /// It internally creates a temporary, dummy image that never has memory bound.
    /// It is just a convenience function, equivalent to calling:
    ///
    /// - `ash::vk::Device::create_image`
    /// - `ash::vk::Device::get_image_memory_requirements`
    /// - `Allocator::find_memory_type_index`
    /// - `ash::vk::Device::destroy_image`
    pub unsafe fn find_memory_type_index_for_image_info(
        &self,
        image_info: ash::vk::ImageCreateInfo,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<u32> {
        let allocation_create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut memory_type_index: u32 = 0;
        ffi_to_result(ffi::vmaFindMemoryTypeIndexForImageInfo(
            self.0,
            &image_info,
            &allocation_create_info,
            &mut memory_type_index,
        ))?;

        Ok(memory_type_index)
    }

    /// Allocates Vulkan device memory and creates `AllocatorPool` object.
    pub unsafe fn create_pool(
        &self,
        pool_info: &AllocatorPoolCreateInfo,
    ) -> VkResult<AllocatorPool> {
        let mut ffi_pool: ffi::VmaPool = mem::zeroed();
        let create_info = pool_create_info_to_ffi(&pool_info);
        ffi_to_result(ffi::vmaCreatePool(self.0, &create_info, &mut ffi_pool))?;
        Ok(AllocatorPool(ffi_pool as _))
    }

    /// Destroys `AllocatorPool` object and frees Vulkan device memory.
    pub unsafe fn destroy_pool(&self, pool: AllocatorPool) {
        ffi::vmaDestroyPool(self.0, pool.0 as *mut _);
    }

    /// Retrieves statistics of existing `AllocatorPool` object.
    pub unsafe fn get_pool_stats(&self, pool: AllocatorPool) -> VkResult<ffi::VmaPoolStats> {
        let mut pool_stats: ffi::VmaPoolStats = mem::zeroed();
        ffi::vmaGetPoolStats(self.0, pool.0 as *mut _, &mut pool_stats);
        Ok(pool_stats)
    }

    /// Marks all allocations in given pool as lost if they are not used in current frame
    /// or AllocatorPoolCreateInfo::frame_in_use_count` back from now.
    ///
    /// Returns the number of allocations marked as lost.
    pub unsafe fn make_pool_allocations_lost(&self, pool: AllocatorPool) -> VkResult<usize> {
        let mut lost_count: usize = 0;
        ffi::vmaMakePoolAllocationsLost(self.0, pool.0 as *mut _, &mut lost_count);
        Ok(lost_count as usize)
    }

    /// Checks magic number in margins around all allocations in given memory pool in search for corruptions.
    ///
    /// Corruption detection is enabled only when `VMA_DEBUG_DETECT_CORRUPTION` macro is defined to nonzero,
    /// `VMA_DEBUG_MARGIN` is defined to nonzero and the pool is created in memory type that is
    /// `ash::vk::MemoryPropertyFlags::HOST_VISIBLE` and `ash::vk::MemoryPropertyFlags::HOST_COHERENT`.
    ///
    /// Possible error values:
    ///
    /// - `ash::vk::Result::ERROR_FEATURE_NOT_PRESENT` - corruption detection is not enabled for specified pool.
    /// - `ash::vk::Result::ERROR_VALIDATION_FAILED_EXT` - corruption detection has been performed and found memory corruptions around one of the allocations.
    ///   `VMA_ASSERT` is also fired in that case.
    /// - Other value: Error returned by Vulkan, e.g. memory mapping failure.
    #[cfg(feature = "detect_corruption")]
    pub unsafe fn check_pool_corruption(&self, pool: AllocatorPool) -> VkResult<()> {
        ffi_to_result(ffi::vmaCheckPoolCorruption(self.0, pool))
    }

    /// General purpose memory allocation.
    ///
    /// You should free the memory using `Allocator::free_memory` or 'Allocator::free_memory_pages'.
    ///
    /// It is recommended to use `Allocator::allocate_memory_for_buffer`, `Allocator::allocate_memory_for_image`,
    /// `Allocator::create_buffer`, `Allocator::create_image` instead whenever possible.
    pub unsafe fn allocate_memory(
        &self,
        memory_requirements: &ash::vk::MemoryRequirements,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<(Allocation, AllocationInfo)> {
        let create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut allocation: Allocation = mem::zeroed();
        let mut allocation_info: AllocationInfo = mem::zeroed();
        ffi_to_result(ffi::vmaAllocateMemory(
            self.0,
            memory_requirements,
            &create_info,
            &mut allocation.0,
            &mut allocation_info.0,
        ))?;

        Ok((allocation, allocation_info))
    }

    /// General purpose memory allocation for multiple allocation objects at once.
    ///
    /// You should free the memory using `Allocator::free_memory` or `Allocator::free_memory_pages`.
    ///
    /// Word "pages" is just a suggestion to use this function to allocate pieces of memory needed for sparse binding.
    /// It is just a general purpose allocation function able to make multiple allocations at once.
    /// It may be internally optimized to be more efficient than calling `Allocator::allocate_memory` `allocations.len()` times.
    ///
    /// All allocations are made using same parameters. All of them are created out of the same memory pool and type.
    pub unsafe fn allocate_memory_pages(
        &self,
        memory_requirements: &ash::vk::MemoryRequirements,
        allocation_info: &AllocationCreateInfo,
        allocation_count: usize,
    ) -> VkResult<Vec<(Allocation, AllocationInfo)>> {
        let create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut allocations: Vec<ffi::VmaAllocation> = vec![mem::zeroed(); allocation_count];
        let mut allocation_info: Vec<ffi::VmaAllocationInfo> =
            vec![mem::zeroed(); allocation_count];
        ffi_to_result(ffi::vmaAllocateMemoryPages(
            self.0,
            memory_requirements,
            &create_info,
            allocation_count,
            allocations.as_mut_ptr(),
            allocation_info.as_mut_ptr(),
        ))?;

        let it = allocations.iter().zip(allocation_info.iter());
        let allocations: Vec<(Allocation, AllocationInfo)> = it
            .map(|(alloc, info)| (Allocation(*alloc), AllocationInfo(*info)))
            .collect();

        Ok(allocations)
    }

    /// Buffer specialized memory allocation.
    ///
    /// You should free the memory using `Allocator::free_memory` or 'Allocator::free_memory_pages'.
    pub unsafe fn allocate_memory_for_buffer(
        &self,
        buffer: ash::vk::Buffer,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<(Allocation, AllocationInfo)> {
        let create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut allocation: Allocation = mem::zeroed();
        let mut allocation_info: AllocationInfo = mem::zeroed();
        ffi_to_result(ffi::vmaAllocateMemoryForBuffer(
            self.0,
            buffer,
            &create_info,
            &mut allocation.0,
            &mut allocation_info.0,
        ))?;

        Ok((allocation, allocation_info))
    }

    /// Image specialized memory allocation.
    ///
    /// You should free the memory using `Allocator::free_memory` or 'Allocator::free_memory_pages'.
    pub unsafe fn allocate_memory_for_image(
        &self,
        image: ash::vk::Image,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<(Allocation, AllocationInfo)> {
        let create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut allocation: Allocation = mem::zeroed();
        let mut allocation_info: AllocationInfo = mem::zeroed();
        ffi_to_result(ffi::vmaAllocateMemoryForImage(
            self.0,
            image,
            &create_info,
            &mut allocation.0,
            &mut allocation_info.0,
        ))?;

        Ok((allocation, allocation_info))
    }

    /// Frees memory previously allocated using `Allocator::allocate_memory`,
    /// `Allocator::allocate_memory_for_buffer`, or `Allocator::allocate_memory_for_image`.
    pub unsafe fn free_memory(&self, allocation: Allocation) {
        ffi::vmaFreeMemory(self.0, allocation.0);
    }

    /// Frees memory and destroys multiple allocations.
    ///
    /// Word "pages" is just a suggestion to use this function to free pieces of memory used for sparse binding.
    /// It is just a general purpose function to free memory and destroy allocations made using e.g. `Allocator::allocate_memory',
    /// 'Allocator::allocate_memory_pages` and other functions.
    ///
    /// It may be internally optimized to be more efficient than calling 'Allocator::free_memory` `allocations.len()` times.
    ///
    /// Allocations in 'allocations' slice can come from any memory pools and types.
    pub unsafe fn free_memory_pages(&self, allocations: &[Allocation]) {
        ffi::vmaFreeMemoryPages(self.0, allocations.len(), allocations.as_ptr() as *mut _);
    }

    /// Returns current information about specified allocation and atomically marks it as used in current frame.
    ///
    /// Current parameters of given allocation are returned in the result object, available through accessors.
    ///
    /// This function also atomically "touches" allocation - marks it as used in current frame,
    /// just like `Allocator::touch_allocation`.
    ///
    /// If the allocation is in lost state, `allocation.get_device_memory` returns `ash::vk::DeviceMemory::null()`.
    ///
    /// Although this function uses atomics and doesn't lock any mutex, so it should be quite efficient,
    /// you can avoid calling it too often.
    ///
    /// If you just want to check if allocation is not lost, `Allocator::touch_allocation` will work faster.
    pub unsafe fn get_allocation_info(&self, allocation: Allocation) -> VkResult<AllocationInfo> {
        let mut allocation_info: AllocationInfo = mem::zeroed();
        ffi::vmaGetAllocationInfo(self.0, allocation.0, &mut allocation_info.0);
        Ok(allocation_info)
    }

    /// Returns `true` if allocation is not lost and atomically marks it as used in current frame.
    ///
    /// If the allocation has been created with `AllocationCreateFlags::CAN_BECOME_LOST` flag,
    /// this function returns `true` if it's not in lost state, so it can still be used.
    /// It then also atomically "touches" the allocation - marks it as used in current frame,
    /// so that you can be sure it won't become lost in current frame or next `frame_in_use_count` frames.
    ///
    /// If the allocation is in lost state, the function returns `false`.
    /// Memory of such allocation, as well as buffer or image bound to it, should not be used.
    /// Lost allocation and the buffer/image still need to be destroyed.
    ///
    /// If the allocation has been created without `AllocationCreateFlags::CAN_BECOME_LOST` flag,
    /// this function always returns `true`.
    pub unsafe fn touch_allocation(&self, allocation: Allocation) -> VkResult<bool> {
        let result = ffi::vmaTouchAllocation(self.0, allocation.0);
        Ok(result == ash::vk::TRUE)
    }

    /// Sets user data in given allocation to new value.
    ///
    /// If the allocation was created with `AllocationCreateFlags::USER_DATA_COPY_STRING`,
    /// `user_data` must be either null, or pointer to a null-terminated string. The function
    /// makes local copy of the string and sets it as allocation's user data. String
    /// passed as user data doesn't need to be valid for whole lifetime of the allocation -
    /// you can free it after this call. String previously pointed by allocation's
    /// user data is freed from memory.
    ///
    /// If the flag was not used, the value of pointer `user_data` is just copied to
    /// allocation's user data. It is opaque, so you can use it however you want - e.g.
    /// as a pointer, ordinal number or some handle to you own data.
    pub unsafe fn set_allocation_user_data(
        &self,
        allocation: Allocation,
        user_data: *mut ::std::os::raw::c_void,
    ) {
        ffi::vmaSetAllocationUserData(self.0, allocation.0, user_data);
    }

    /// Creates new allocation that is in lost state from the beginning.
    ///
    /// It can be useful if you need a dummy, non-null allocation.
    ///
    /// You still need to destroy created object using `Allocator::free_memory`.
    ///
    /// Returned allocation is not tied to any specific memory pool or memory type and
    /// not bound to any image or buffer. It has size = 0. It cannot be turned into
    /// a real, non-empty allocation.
    pub unsafe fn create_lost_allocation(&self) -> VkResult<Allocation> {
        let mut allocation: Allocation = mem::zeroed();
        ffi::vmaCreateLostAllocation(self.0, &mut allocation.0);
        Ok(allocation)
    }

    /// Maps memory represented by given allocation and returns pointer to it.
    ///
    /// Maps memory represented by given allocation to make it accessible to CPU code.
    /// When succeeded, result is a pointer to first byte of this memory.
    ///
    /// If the allocation is part of bigger `ash::vk::DeviceMemory` block, the pointer is
    /// correctly offseted to the beginning of region assigned to this particular
    /// allocation.
    ///
    /// Mapping is internally reference-counted and synchronized, so despite raw Vulkan
    /// function `ash::vk::Device::MapMemory` cannot be used to map same block of
    /// `ash::vk::DeviceMemory` multiple times simultaneously, it is safe to call this
    /// function on allocations assigned to the same memory block. Actual Vulkan memory
    /// will be mapped on first mapping and unmapped on last unmapping.
    ///
    /// If the function succeeded, you must call `Allocator::unmap_memory` to unmap the
    /// allocation when mapping is no longer needed or before freeing the allocation, at
    /// the latest.
    ///
    /// It also safe to call this function multiple times on the same allocation. You
    /// must call `Allocator::unmap_memory` same number of times as you called
    /// `Allocator::map_memory`.
    ///
    /// It is also safe to call this function on allocation created with
    /// `AllocationCreateFlags::MAPPED` flag. Its memory stays mapped all the time.
    /// You must still call `Allocator::unmap_memory` same number of times as you called
    /// `Allocator::map_memory`. You must not call `Allocator::unmap_memory` additional
    /// time to free the "0-th" mapping made automatically due to `AllocationCreateFlags::MAPPED` flag.
    ///
    /// This function fails when used on allocation made in memory type that is not
    /// `ash::vk::MemoryPropertyFlags::HOST_VISIBLE`.
    ///
    /// This function always fails when called for allocation that was created with
    /// `AllocationCreateFlags::CAN_BECOME_LOST` flag. Such allocations cannot be mapped.
    pub unsafe fn map_memory(&self, allocation: Allocation) -> VkResult<*mut u8> {
        let mut mapped_data: *mut ::std::os::raw::c_void = ::std::ptr::null_mut();
        ffi_to_result(ffi::vmaMapMemory(self.0, allocation.0, &mut mapped_data))?;

        Ok(mapped_data as *mut u8)
    }

    /// Unmaps memory represented by given allocation, mapped previously using `Allocator::map_memory`.
    pub unsafe fn unmap_memory(&self, allocation: Allocation) {
        ffi::vmaUnmapMemory(self.0, allocation.0);
    }

    /// Flushes memory of given allocation.
    ///
    /// Calls `ash::vk::Device::FlushMappedMemoryRanges` for memory associated with given range of given allocation.
    ///
    /// - `offset` must be relative to the beginning of allocation.
    /// - `size` can be `ash::vk::WHOLE_SIZE`. It means all memory from `offset` the the end of given allocation.
    /// - `offset` and `size` don't have to be aligned; hey are internally rounded down/up to multiple of `nonCoherentAtomSize`.
    /// - If `size` is 0, this call is ignored.
    /// - If memory type that the `allocation` belongs to is not `ash::vk::MemoryPropertyFlags::HOST_VISIBLE` or it is `ash::vk::MemoryPropertyFlags::HOST_COHERENT`, this call is ignored.
    pub unsafe fn flush_allocation(
        &self,
        allocation: Allocation,
        offset: usize,
        size: usize,
    ) -> VkResult<()> {
        ffi_to_result(ffi::vmaFlushAllocation(
            self.0,
            allocation.0,
            offset as vk::DeviceSize,
            size as vk::DeviceSize,
        ))
    }

    /// Invalidates memory of given allocation.
    ///
    /// Calls `ash::vk::Device::invalidate_mapped_memory_ranges` for memory associated with given range of given allocation.
    ///
    /// - `offset` must be relative to the beginning of allocation.
    /// - `size` can be `ash::vk::WHOLE_SIZE`. It means all memory from `offset` the the end of given allocation.
    /// - `offset` and `size` don't have to be aligned. They are internally rounded down/up to multiple of `nonCoherentAtomSize`.
    /// - If `size` is 0, this call is ignored.
    /// - If memory type that the `allocation` belongs to is not `ash::vk::MemoryPropertyFlags::HOST_VISIBLE` or it is `ash::vk::MemoryPropertyFlags::HOST_COHERENT`, this call is ignored.
    pub unsafe fn invalidate_allocation(
        &self,
        allocation: Allocation,
        offset: usize,
        size: usize,
    ) -> VkResult<()> {
        ffi_to_result(ffi::vmaInvalidateAllocation(
            self.0,
            allocation.0,
            offset as vk::DeviceSize,
            size as vk::DeviceSize,
        ))
    }

    /// Checks magic number in margins around all allocations in given memory types (in both default and custom pools) in search for corruptions.
    ///
    /// `memory_type_bits` bit mask, where each bit set means that a memory type with that index should be checked.
    ///
    /// Corruption detection is enabled only when `VMA_DEBUG_DETECT_CORRUPTION` macro is defined to nonzero,
    /// `VMA_DEBUG_MARGIN` is defined to nonzero and only for memory types that are `HOST_VISIBLE` and `HOST_COHERENT`.
    ///
    /// Possible error values:
    ///
    /// - `ash::vk::Result::ERROR_FEATURE_NOT_PRESENT` - corruption detection is not enabled for any of specified memory types.
    /// - `ash::vk::Result::ERROR_VALIDATION_FAILED_EXT` - corruption detection has been performed and found memory corruptions around one of the allocations.
    ///   `VMA_ASSERT` is also fired in that case.
    /// - Other value: Error returned by Vulkan, e.g. memory mapping failure.
    #[cfg(feature = "detect_corruption")]
    pub unsafe fn check_corruption(
        &self,
        memory_types: ash::vk::MemoryPropertyFlags,
    ) -> VkResult<()> {
        ffi_to_result(ffi::vmaCheckCorruption(self.0, memory_types.as_raw()))
    }

    /// Begins defragmentation process.
    ///
    /// Use this function instead of old, deprecated `Allocator::defragment`.
    ///
    /// Warning! Between the call to `Allocator::defragmentation_begin` and `Allocator::defragmentation_end`.
    ///
    /// - You should not use any of allocations passed as `allocations` or
    /// any allocations that belong to pools passed as `pools`,
    /// including calling `Allocator::get_allocation_info`, `Allocator::touch_allocation`, or access
    /// their data.
    ///
    /// - Some mutexes protecting internal data structures may be locked, so trying to
    /// make or free any allocations, bind buffers or images, map memory, or launch
    /// another simultaneous defragmentation in between may cause stall (when done on
    /// another thread) or deadlock (when done on the same thread), unless you are
    /// 100% sure that defragmented allocations are in different pools.
    ///
    /// - Information returned via stats and `info.allocations_changed` are undefined.
    /// They become valid after call to `Allocator::defragmentation_end`.
    ///
    /// - If `info.command_buffer` is not null, you must submit that command buffer
    /// and make sure it finished execution before calling `Allocator::defragmentation_end`.
    pub unsafe fn defragmentation_begin(
        &self,
        info: &DefragmentationInfo2,
    ) -> VkResult<DefragmentationContext> {
        let command_buffer = match info.command_buffer {
            Some(command_buffer) => command_buffer,
            None => ash::vk::CommandBuffer::null(),
        };

        let mut context = DefragmentationContext {
            internal: mem::zeroed(),
            stats: ffi::VmaDefragmentationStats {
                bytesMoved: 0,
                bytesFreed: 0,
                allocationsMoved: 0,
                deviceMemoryBlocksFreed: 0,
            },
            changed: vec![ash::vk::FALSE; info.allocations.len()],
        };

        let pools = info.pools.unwrap_or(&[]);

        let ffi_info = ffi::VmaDefragmentationInfo2 {
            flags: 0, // Reserved for future use
            allocationCount: info.allocations.len() as u32,
            pAllocations: info.allocations.as_ptr() as *mut _,
            pAllocationsChanged: context.changed.as_mut_ptr(),
            poolCount: pools.len() as u32,
            pPools: pools.as_ptr() as *mut _,
            maxCpuBytesToMove: info.max_cpu_bytes_to_move,
            maxCpuAllocationsToMove: info.max_cpu_allocations_to_move,
            maxGpuBytesToMove: info.max_gpu_bytes_to_move,
            maxGpuAllocationsToMove: info.max_gpu_allocations_to_move,
            commandBuffer: command_buffer,
        };

        ffi_to_result(ffi::vmaDefragmentationBegin(
            self.0,
            &ffi_info,
            &mut context.stats as *mut _,
            &mut context.internal,
        ))?;

        Ok(context)
    }

    /// Ends defragmentation process.
    ///
    /// Use this function to finish defragmentation started by `Allocator::defragmentation_begin`.
    pub unsafe fn defragmentation_end(
        &self,
        context: &mut DefragmentationContext,
    ) -> VkResult<(DefragmentationStats, Vec<bool>)> {
        ffi_to_result(ffi::vmaDefragmentationEnd(self.0, context.internal))?;

        let changed: Vec<bool> = context.changed.iter().map(|change| *change == 1).collect();

        let stats = DefragmentationStats {
            bytes_moved: context.stats.bytesMoved as usize,
            bytes_freed: context.stats.bytesFreed as usize,
            allocations_moved: context.stats.allocationsMoved,
            device_memory_blocks_freed: context.stats.deviceMemoryBlocksFreed,
        };

        Ok((stats, changed))
    }

    /// Compacts memory by moving allocations.
    ///
    /// `allocations` is a slice of allocations that can be moved during this compaction.
    /// `defrag_info` optional configuration parameters.
    /// Returns statistics from the defragmentation, and an associated array to `allocations`
    /// which indicates which allocations were changed (if any).
    ///
    /// Possible error values:
    ///
    /// - `ash::vk::Result::INCOMPLETE` if succeeded but didn't make all possible optimizations because limits specified in
    ///   `defrag_info` have been reached, negative error code in case of error.
    ///
    /// This function works by moving allocations to different places (different
    /// `ash::vk::DeviceMemory` objects and/or different offsets) in order to optimize memory
    /// usage. Only allocations that are in `allocations` slice can be moved. All other
    /// allocations are considered nonmovable in this call. Basic rules:
    ///
    /// - Only allocations made in memory types that have
    ///   `ash::vk::MemoryPropertyFlags::HOST_VISIBLE` and `ash::vk::MemoryPropertyFlags::HOST_COHERENT`
    ///   flags can be compacted. You may pass other allocations but it makes no sense -
    ///   these will never be moved.
    ///
    /// - Custom pools created with `AllocatorPoolCreateFlags::LINEAR_ALGORITHM` or `AllocatorPoolCreateFlags::BUDDY_ALGORITHM` flag are not
    ///   defragmented. Allocations passed to this function that come from such pools are ignored.
    ///
    /// - Allocations created with `AllocationCreateFlags::DEDICATED_MEMORY` or created as dedicated allocations for any
    ///   other reason are also ignored.
    ///
    /// - Both allocations made with or without `AllocationCreateFlags::MAPPED` flag can be compacted. If not persistently
    ///   mapped, memory will be mapped temporarily inside this function if needed.
    ///
    /// - You must not pass same `allocation` object multiple times in `allocations` slice.
    ///
    /// The function also frees empty `ash::vk::DeviceMemory` blocks.
    ///
    /// Warning: This function may be time-consuming, so you shouldn't call it too often
    /// (like after every resource creation/destruction).
    /// You can call it on special occasions (like when reloading a game level or
    /// when you just destroyed a lot of objects). Calling it every frame may be OK, but
    /// you should measure that on your platform.
    #[deprecated(
        since = "0.1.3",
        note = "This is a part of the old interface. It is recommended to use structure `DefragmentationInfo2` and function `Allocator::defragmentation_begin` instead."
    )]
    pub unsafe fn defragment(
        &self,
        allocations: &[Allocation],
        defrag_info: Option<&DefragmentationInfo>,
    ) -> VkResult<(DefragmentationStats, Vec<bool>)> {
        let mut ffi_change_list: Vec<vk::Bool32> = vec![0; allocations.len()];
        let ffi_info = match defrag_info {
            Some(info) => ffi::VmaDefragmentationInfo {
                maxBytesToMove: info.max_bytes_to_move as vk::DeviceSize,
                maxAllocationsToMove: info.max_allocations_to_move,
            },
            None => ffi::VmaDefragmentationInfo {
                maxBytesToMove: ash::vk::WHOLE_SIZE,
                maxAllocationsToMove: std::u32::MAX,
            },
        };

        let mut ffi_stats: ffi::VmaDefragmentationStats = mem::zeroed();
        ffi_to_result(ffi::vmaDefragment(
            self.0,
            allocations.as_ptr() as *mut _,
            allocations.len(),
            ffi_change_list.as_mut_ptr(),
            &ffi_info,
            &mut ffi_stats,
        ))?;

        let change_list: Vec<bool> = ffi_change_list
            .iter()
            .map(|change| *change == ash::vk::TRUE)
            .collect();

        let stats = DefragmentationStats {
            bytes_moved: ffi_stats.bytesMoved as usize,
            bytes_freed: ffi_stats.bytesFreed as usize,
            allocations_moved: ffi_stats.allocationsMoved,
            device_memory_blocks_freed: ffi_stats.deviceMemoryBlocksFreed,
        };

        Ok((stats, change_list))
    }

    /// Binds buffer to allocation.
    ///
    /// Binds specified buffer to region of memory represented by specified allocation.
    /// Gets `ash::vk::DeviceMemory` handle and offset from the allocation.
    ///
    /// If you want to create a buffer, allocate memory for it and bind them together separately,
    /// you should use this function for binding instead of `ash::vk::Device::bind_buffer_memory`,
    /// because it ensures proper synchronization so that when a `ash::vk::DeviceMemory` object is
    /// used by multiple allocations, calls to `ash::vk::Device::bind_buffer_memory()` or
    /// `ash::vk::Device::map_memory()` won't happen from multiple threads simultaneously
    /// (which is illegal in Vulkan).
    ///
    /// It is recommended to use function `Allocator::create_buffer` instead of this one.
    pub unsafe fn bind_buffer_memory(
        &self,
        buffer: ash::vk::Buffer,
        allocation: Allocation,
    ) -> VkResult<()> {
        ffi_to_result(ffi::vmaBindBufferMemory(self.0, allocation.0, buffer))
    }

    /// Binds image to allocation.
    ///
    /// Binds specified image to region of memory represented by specified allocation.
    /// Gets `ash::vk::DeviceMemory` handle and offset from the allocation.
    ///
    /// If you want to create a image, allocate memory for it and bind them together separately,
    /// you should use this function for binding instead of `ash::vk::Device::bind_image_memory`,
    /// because it ensures proper synchronization so that when a `ash::vk::DeviceMemory` object is
    /// used by multiple allocations, calls to `ash::vk::Device::bind_image_memory()` or
    /// `ash::vk::Device::map_memory()` won't happen from multiple threads simultaneously
    /// (which is illegal in Vulkan).
    ///
    /// It is recommended to use function `Allocator::create_image` instead of this one.
    pub unsafe fn bind_image_memory(
        &self,
        image: ash::vk::Image,
        allocation: Allocation,
    ) -> VkResult<()> {
        ffi_to_result(ffi::vmaBindImageMemory(self.0, allocation.0, image))
    }

    /// This function automatically creates a buffer, allocates appropriate memory
    /// for it, and binds the buffer with the memory.
    ///
    /// If the function succeeded, you must destroy both buffer and allocation when you
    /// no longer need them using either convenience function `Allocator::destroy_buffer` or
    /// separately, using `ash::Device::destroy_buffer` and `Allocator::free_memory`.
    ///
    /// If `AllocatorCreateFlags::KHR_DEDICATED_ALLOCATION` flag was used,
    /// VK_KHR_dedicated_allocation extension is used internally to query driver whether
    /// it requires or prefers the new buffer to have dedicated allocation. If yes,
    /// and if dedicated allocation is possible (AllocationCreateInfo::pool is null
    /// and `AllocationCreateFlags::NEVER_ALLOCATE` is not used), it creates dedicated
    /// allocation for this buffer, just like when using `AllocationCreateFlags::DEDICATED_MEMORY`.
    pub unsafe fn create_buffer(
        &self,
        buffer_info: &ash::vk::BufferCreateInfo,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<(ash::vk::Buffer, Allocation, AllocationInfo)> {
        let allocation_create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut buffer = vk::Buffer::null();
        let mut allocation: Allocation = mem::zeroed();
        let mut allocation_info: AllocationInfo = mem::zeroed();
        ffi_to_result(ffi::vmaCreateBuffer(
            self.0,
            &*buffer_info,
            &allocation_create_info,
            &mut buffer,
            &mut allocation.0,
            &mut allocation_info.0,
        ))?;

        Ok((buffer, allocation, allocation_info))
    }

    /// Destroys Vulkan buffer and frees allocated memory.
    ///
    /// This is just a convenience function equivalent to:
    ///
    /// ```text
    /// ash::vk::Device::destroy_buffer(buffer, None);
    /// Allocator::free_memory(allocator, allocation);
    /// ```
    ///
    /// It it safe to pass null as `buffer` and/or `allocation`.
    pub unsafe fn destroy_buffer(&self, buffer: ash::vk::Buffer, allocation: Allocation) {
        ffi::vmaDestroyBuffer(self.0, buffer, allocation.0);
    }

    /// This function automatically creates an image, allocates appropriate memory
    /// for it, and binds the image with the memory.
    ///
    /// If the function succeeded, you must destroy both image and allocation when you
    /// no longer need them using either convenience function `Allocator::destroy_image` or
    /// separately, using `ash::Device::destroy_image` and `Allocator::free_memory`.
    ///
    /// If `AllocatorCreateFlags::KHR_DEDICATED_ALLOCATION` flag was used,
    /// `VK_KHR_dedicated_allocation extension` is used internally to query driver whether
    /// it requires or prefers the new image to have dedicated allocation. If yes,
    /// and if dedicated allocation is possible (AllocationCreateInfo::pool is null
    /// and `AllocationCreateFlags::NEVER_ALLOCATE` is not used), it creates dedicated
    /// allocation for this image, just like when using `AllocationCreateFlags::DEDICATED_MEMORY`.
    ///
    /// If `VK_ERROR_VALIDAITON_FAILED_EXT` is returned, VMA may have encountered a problem
    /// that is not caught by the validation layers. One example is if you try to create a 0x0
    /// image, a panic will occur and `VK_ERROR_VALIDAITON_FAILED_EXT` is thrown.
    pub unsafe fn create_image(
        &self,
        image_info: &ash::vk::ImageCreateInfo,
        allocation_info: &AllocationCreateInfo,
    ) -> VkResult<(ash::vk::Image, Allocation, AllocationInfo)> {
        let allocation_create_info = allocation_create_info_to_ffi(&allocation_info);
        let mut image = vk::Image::null();
        let mut allocation: Allocation = mem::zeroed();
        let mut allocation_info: AllocationInfo = mem::zeroed();
        ffi_to_result(ffi::vmaCreateImage(
            self.0,
            &*image_info,
            &allocation_create_info,
            &mut image,
            &mut allocation.0,
            &mut allocation_info.0,
        ))?;

        Ok((image, allocation, allocation_info))
    }

    /// Destroys Vulkan image and frees allocated memory.
    ///
    /// This is just a convenience function equivalent to:
    ///
    /// ```text
    /// ash::vk::Device::destroy_image(image, None);
    /// Allocator::free_memory(allocator, allocation);
    /// ```
    ///
    /// It it safe to pass null as `image` and/or `allocation`.
    pub unsafe fn destroy_image(&self, image: ash::vk::Image, allocation: Allocation) {
        ffi::vmaDestroyImage(self.0, image, allocation.0);
    }

    /// Destroys the internal allocator instance. After this has been called,
    /// no other functions may be called. Useful for ensuring a specific destruction
    /// order (for example, if an Allocator is a member of something that owns the Vulkan
    /// instance and destroys it in its own Drop).
    pub unsafe fn destroy_allocator(&self) {
        ffi::vmaDestroyAllocator(self.0);
    }
}
