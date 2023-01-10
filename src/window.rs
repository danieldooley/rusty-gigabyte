use std::sync::mpsc::Receiver;
use speedy2d::color::Color;
use speedy2d::Graphics2D;
use speedy2d::image::{ImageDataType, ImageSmoothingMode};
use speedy2d::shape::Rectangle;
use speedy2d::window::{WindowHandler, WindowHelper};

pub struct GBWindowHandler {
    receiver: Receiver<Vec<u8>>
}

pub fn new_gb_window_handler(receiver: Receiver<Vec<u8>>) -> GBWindowHandler {
    GBWindowHandler {
        receiver,
    }
}

impl WindowHandler for GBWindowHandler {
    fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D)
    {
        match self.receiver.try_recv() {
            Ok(fb) => {
                let image = graphics.create_image_from_raw_pixels(ImageDataType::RGB, ImageSmoothingMode::NearestNeighbor, (160, 144), &fb).unwrap();

                graphics.draw_rectangle_image(Rectangle::from_tuples((0.0, 0.0), (1280.0, 1152.0)), &image);

                // Request that we draw another frame once this one has finished
                helper.request_redraw();
            }
            _ => {} // TODO: Handle disconnection
        }
    }
}