use std::sync::Arc;
use std::sync::mpsc::Receiver;
use speedy2d::color::Color;
use speedy2d::dimen::UVec2;
use speedy2d::Graphics2D;
use speedy2d::image::{ImageDataType, ImageSmoothingMode};
use speedy2d::shape::Rectangle;
use speedy2d::window::{KeyScancode, VirtualKeyCode, WindowHandler, WindowHelper};
use crate::gameboy::keys::{KeyReg, Keys};

pub struct GBWindowHandler {
    size: UVec2,

    key_reg: Arc<KeyReg>,

    frame: Vec<u8>,
}

pub fn new_gb_window_handler(key_reg: Arc<KeyReg>) -> GBWindowHandler {
    GBWindowHandler {
        size: UVec2::from((160, 144)),

        key_reg,

        frame: vec!(),
    }
}

impl GBWindowHandler {
    fn map_vkc_to_key(&self, scancode: Option<VirtualKeyCode>) -> Option<Keys> {
        match scancode {
            Some(VirtualKeyCode::Return) => Some(Keys::START), // Enter
            Some(VirtualKeyCode::Space) => Some(Keys::SELECT), // Space
            Some(VirtualKeyCode::Left) => Some(Keys::LEFT), // Left Arrow
            Some(VirtualKeyCode::Up) => Some(Keys::UP), // Up Arrow
            Some(VirtualKeyCode::Right) => Some(Keys::RIGHT), // Right Arrow
            Some(VirtualKeyCode::Down) => Some(Keys::DOWN), // Down Arrow
            Some(VirtualKeyCode::S) => Some(Keys::B), // X
            Some(VirtualKeyCode::A) => Some(Keys::A), // Z
            _ => None,
        }
    }
}

impl WindowHandler<Vec<u8>> for GBWindowHandler {
    fn on_user_event(&mut self, helper: &mut WindowHelper<Vec<u8>>, user_event: Vec<u8>) {
        self.frame = user_event;

        helper.request_redraw();
    }

    fn on_key_down(&mut self, helper: &mut WindowHelper<Vec<u8>>, virtual_key_code: Option<VirtualKeyCode>, scancode: KeyScancode) {
        match self.map_vkc_to_key(virtual_key_code) {
            None => {}
            Some(k) => self.key_reg.key_down(k)
        }
    }

    fn on_key_up(&mut self, helper: &mut WindowHelper<Vec<u8>>, virtual_key_code: Option<VirtualKeyCode>, scancode: KeyScancode) {
        match self.map_vkc_to_key(virtual_key_code) {
            None => {}
            Some(k) => self.key_reg.key_up(k)
        }
    }

    fn on_resize(&mut self, helper: &mut WindowHelper<Vec<u8>>, size_pixels: UVec2) {
        self.size = size_pixels;

        helper.request_redraw();
    }

    fn on_draw(&mut self, helper: &mut WindowHelper<Vec<u8>>, graphics: &mut Graphics2D)
    {
        let image = graphics.create_image_from_raw_pixels(ImageDataType::RGB, ImageSmoothingMode::NearestNeighbor, (160, 144), &self.frame).unwrap();

        graphics.draw_rectangle_image(Rectangle::from_tuples((0.0, 0.0), (self.size.x as f32, self.size.y as f32)), &image);
    }
}