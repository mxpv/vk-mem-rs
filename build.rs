use std::env;
use std::path::PathBuf;

fn main() {
    let mut build = cc::Build::new();

    build.include("extern/Vulkan-Headers/include/vulkan");
    build.include("extern/VulkanMemoryAllocator/include");

    // Disable VMA_ASSERT when rust assertions are disabled
    #[cfg(not(debug_assertions))]
    build.define("NDEBUG", "");

    // We want to use the loader in ash, instead of requiring us to link
    // in vulkan.dll/.dylib in addition to ash. This is especially important
    // for MoltenVK, where there is no default installation path, unlike
    // Linux (pkconfig) and Windows (VULKAN_SDK environment variable).
    build.define("VMA_STATIC_VULKAN_FUNCTIONS", "0");

    // This prevents VMA from trying to fetch any remaining pointers
    // that are still null after using the loader in ash, which can
    // cause linker errors.
    build.define("VMA_DYNAMIC_VULKAN_FUNCTIONS", "0");

    // TODO: Add some configuration options under crate features
    //#define VMA_HEAVY_ASSERT(expr) assert(expr)
    //#define VMA_USE_STL_CONTAINERS 1
    //#define VMA_DEDICATED_ALLOCATION 0
    //#define VMA_DEBUG_INITIALIZE_ALLOCATIONS 1
    //#define VMA_DEBUG_MIN_BUFFER_IMAGE_GRANULARITY 256

    #[cfg(feature = "detect_corruption")]
    {
        build.define("VMA_DEBUG_DETECT_CORRUPTION", "1");
        build.define("VMA_DEBUG_MARGIN", "16");
    }

    #[cfg(feature = "recording")]
    build.define("VMA_RECORDING_ENABLED", "1");

    // Add the files we build
    build.file("src/vma.cpp");

    let target = env::var("TARGET").unwrap();
    if target.contains("darwin") {
        build
            .flag("-std=c++11")
            .flag("-Wno-missing-field-initializers")
            .flag("-Wno-unused-variable")
            .flag("-Wno-unused-parameter")
            .flag("-Wno-unused-private-field")
            .flag("-Wno-reorder")
            .flag("-Wno-nullability-completeness")
            .cpp_link_stdlib("c++")
            .cpp_set_stdlib("c++")
            .cpp(true);
    } else if target.contains("ios") {
        build
            .flag("-std=c++11")
            .flag("-Wno-missing-field-initializers")
            .flag("-Wno-unused-variable")
            .flag("-Wno-unused-parameter")
            .flag("-Wno-unused-private-field")
            .flag("-Wno-reorder")
            .cpp_link_stdlib("c++")
            .cpp_set_stdlib("c++")
            .cpp(true);
    } else if target.contains("android") {
        build
            .flag("-std=c++11")
            .flag("-Wno-missing-field-initializers")
            .flag("-Wno-unused-variable")
            .flag("-Wno-unused-parameter")
            .flag("-Wno-unused-private-field")
            .flag("-Wno-reorder")
            .cpp_link_stdlib("c++")
            .cpp(true);
    } else if target.contains("linux") {
        build
            .flag("-std=c++11")
            .flag("-Wno-missing-field-initializers")
            .flag("-Wno-unused-variable")
            .flag("-Wno-unused-parameter")
            .flag("-Wno-unused-private-field")
            .flag("-Wno-reorder")
            .cpp_link_stdlib("stdc++")
            .cpp(true);
    } else if target.contains("windows") && target.contains("gnu") {
        build
            .flag("-std=c++11")
            .flag("-Wno-missing-field-initializers")
            .flag("-Wno-unused-variable")
            .flag("-Wno-unused-parameter")
            .flag("-Wno-unused-private-field")
            .flag("-Wno-reorder")
            .flag("-Wno-type-limits")
            .cpp_link_stdlib("stdc++")
            .cpp(true);
    }

    build.compile("vma_cpp");

    link_vulkan();
    generate_bindings();
}

#[cfg(feature = "link_vulkan")]
fn link_vulkan() {
    let target = env::var("TARGET").unwrap();
    if target.contains("windows") {
        if let Ok(vulkan_sdk) = env::var("VULKAN_SDK") {
            let mut vulkan_sdk_path = PathBuf::from(vulkan_sdk);

            if target.contains("x86_64") {
                vulkan_sdk_path.push("Lib");
            } else {
                vulkan_sdk_path.push("Lib32");
            }

            println!(
                "cargo:rustc-link-search=native={}",
                vulkan_sdk_path.to_str().unwrap()
            );
        }

        println!("cargo:rustc-link-lib=dylib=vulkan-1");
    } else {
        if target.contains("apple") {
            if let Ok(vulkan_sdk) = env::var("VULKAN_SDK") {
                let mut vulkan_sdk_path = PathBuf::from(vulkan_sdk);
                vulkan_sdk_path.push("macOS/lib");
                println!(
                    "cargo:rustc-link-search=native={}",
                    vulkan_sdk_path.to_str().unwrap()
                );
            } else {
                let lib_path = "wrapper/macOS/lib";
                println!("cargo:rustc-link-search=native={}", lib_path);
            }

            println!("cargo:rustc-link-lib=dylib=vulkan");
        }
    }
}

#[cfg(not(feature = "link_vulkan"))]
fn link_vulkan() {}

fn generate_bindings() {
    let bindings = bindgen::Builder::default()
        .clang_arg("-I./wrapper")
        .header("extern/VulkanMemoryAllocator/include/vk_mem_alloc.h")
        .rustfmt_bindings(true)
        .size_t_is_usize(true)
        .blocklist_type("__darwin_.*")
        .allowlist_function("vma.*")
        .parse_callbacks(Box::new(FixAshTypes))
        .blocklist_type("Vk.*")
        .blocklist_type("PFN_vk.*")
        .raw_line("use ash::vk::*;")
        .trust_clang_mangling(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate bindings!");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Unable to write bindings!");
}

#[derive(Debug)]
struct FixAshTypes;

impl bindgen::callbacks::ParseCallbacks for FixAshTypes {
    fn item_name(&self, original_item_name: &str) -> Option<String> {
        if original_item_name.starts_with("Vk") {
            // Strip `Vk` prefix, will use `ash::vk::*` instead
            Some(original_item_name.trim_start_matches("Vk").to_string())
        } else if original_item_name.starts_with("PFN_vk") && original_item_name.ends_with("KHR") {
            // VMA uses a few extensions like `PFN_vkGetBufferMemoryRequirements2KHR`,
            // ash keeps these as `PFN_vkGetBufferMemoryRequirements2`
            Some(original_item_name.trim_end_matches("KHR").to_string())
        } else {
            None
        }
    }

    // When ignoring `Vk` types, bindgen loses derives for some type. Quick workaround.
    fn add_derives(&self, name: &str) -> Vec<String> {
        if name.starts_with("VmaAllocationInfo") || name.starts_with("VmaDefragmentationStats") {
            vec!["Debug".into(), "Copy".into(), "Clone".into()]
        } else {
            vec![]
        }
    }
}
