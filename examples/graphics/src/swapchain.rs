use crate::context::*;
use std::cmp;
use std::slice;
use std::sync::Arc;
use std::u64;
use vkr::{vk, Builder};

pub struct Swapchain {
    context: Arc<Context>,
    swapchain: vk::SwapchainKHR,
    extent: vk::Extent2D,
    images: Vec<vk::Image>,
}

pub enum SwapchainAcquireResult {
    RecreateNow,
    RecreateSoon(u32),
    Ok(u32),
}

impl Swapchain {
    pub const FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;
    pub const COLOR_SPACE: vk::ColorSpaceKHR = vk::ColorSpaceKHR::SRGB_NONLINEAR;

    const MIN_IMAGE_COUNT: u32 = 2;

    fn create(
        context: &Context,
        usage: vk::ImageUsageFlags,
        old_swapchain: Option<vk::SwapchainKHR>,
    ) -> (vk::SwapchainKHR, vk::Extent2D) {
        let surface_capabilities = unsafe {
            context
                .instance
                .get_physical_device_surface_capabilities_khr(context.physical_device, context.surface)
        }
        .unwrap();
        let extent = surface_capabilities.current_extent;
        let surface_supported = unsafe {
            context.instance.get_physical_device_surface_support_khr(
                context.physical_device,
                context.queue_family_index,
                context.surface,
            )
        }
        .unwrap();
        if !surface_supported {
            panic!("swapchain surface not supported");
        }

        let formats = unsafe {
            context
                .instance
                .get_physical_device_surface_formats_khr_to_vec(context.physical_device, context.surface)
        }
        .unwrap();
        let format_supported = formats
            .iter()
            .any(|f| f.format == Self::FORMAT && f.color_space == Self::COLOR_SPACE);
        if !format_supported {
            panic!("swapchain format not supported");
        }

        let min_image_count = cmp::max(Self::MIN_IMAGE_COUNT, surface_capabilities.min_image_count);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(context.surface)
            .min_image_count(min_image_count)
            .image_format(Self::FORMAT)
            .image_color_space(Self::COLOR_SPACE)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(usage)
            .p_queue_family_indices(slice::from_ref(&context.queue_family_index))
            .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true)
            .old_swapchain(old_swapchain);
        let swapchain = unsafe { context.device.create_swapchain_khr(&swapchain_create_info, None) }.unwrap();

        (swapchain, extent)
    }

    pub fn new(context: &Arc<Context>, usage: vk::ImageUsageFlags) -> Self {
        let (swapchain, extent) = Swapchain::create(context, usage, None);

        let images = unsafe { context.device.get_swapchain_images_khr_to_vec(swapchain) }.unwrap();

        Swapchain {
            context: Arc::clone(&context),
            swapchain,
            extent,
            images,
        }
    }

    pub fn recreate(&mut self, usage: vk::ImageUsageFlags) {
        let (swapchain, extent) = Swapchain::create(&self.context, usage, Some(self.swapchain));
        unsafe { self.context.device.destroy_swapchain_khr(Some(self.swapchain), None) };

        self.swapchain = swapchain;
        self.extent = extent;
        self.images = unsafe { self.context.device.get_swapchain_images_khr_to_vec(swapchain) }.unwrap();
    }

    pub fn acquire(&self, image_available_semaphore: vk::Semaphore) -> SwapchainAcquireResult {
        let res = unsafe {
            self.context
                .device
                .acquire_next_image_khr(self.swapchain, u64::MAX, Some(image_available_semaphore), None)
        };
        match res {
            Ok((vk::Result::SUCCESS, image_index)) => SwapchainAcquireResult::Ok(image_index),
            Ok((vk::Result::SUBOPTIMAL_KHR, image_index)) => SwapchainAcquireResult::RecreateSoon(image_index),
            Ok((err, _)) => panic!("failed to acquire next image {}", err),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => SwapchainAcquireResult::RecreateNow,
            Err(err) => panic!("failed to acquire next image {}", err),
        }
    }

    pub fn get_image(&self, image_index: u32) -> vk::Image {
        self.images[image_index as usize]
    }

    pub fn get_extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub fn present(&self, image_index: u32, rendering_finished_semaphore: vk::Semaphore) {
        let present_info = vk::PresentInfoKHR::builder()
            .p_wait_semaphores(slice::from_ref(&rendering_finished_semaphore))
            .p_swapchains(slice::from_ref(&self.swapchain), slice::from_ref(&image_index));
        match unsafe { self.context.device.queue_present_khr(self.context.queue, &present_info) } {
            Ok(vk::Result::SUCCESS) | Ok(vk::Result::SUBOPTIMAL_KHR) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {}
            Ok(err) | Err(err) => panic!("failed to present {}", err),
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            self.context.device.destroy_swapchain_khr(Some(self.swapchain), None);
        }
    }
}