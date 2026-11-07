//! Shared frame damage policy, independent of a window or renderer.
use crate::{DisplayList, PixelRect};
/// Pixel bounds changed since `old`; a new/resized bitmap needs every pixel.
pub fn regions(list: &DisplayList, old: &DisplayList, size: (u32,u32), resized: bool, sparse: bool) -> Vec<PixelRect> {
    if resized { vec![PixelRect::full(size.0, size.1)] }
    else if sparse { list.damage_regions(old).into_iter().filter_map(|r| r.pixels(size.0,size.1)).collect() }
    else { list.damage(old).and_then(|r| r.pixels(size.0,size.1)).into_iter().collect() }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, Rect, Rgba};
    #[test]
    fn resize_invalidates_every_pixel_but_identical_frames_do_nothing() {
        let list = DisplayList::default();
        assert_eq!(regions(&list,&list,(20,10),true,false), vec![PixelRect::full(20,10)]);
        assert!(regions(&list,&list,(20,10),false,false).is_empty());
        assert!(regions(&list,&list,(20,10),false,true).is_empty());
    }
    #[test]
    fn removal_and_addition_are_clamped_to_the_bitmap_in_both_modes() {
        let old = DisplayList { ops: vec![Op::Fill { rect: Rect::new(-10.0,-10.0,20.0,20.0), radius: 0.0, color: Rgba::HIT }], ..Default::default() };
        let empty = DisplayList::default();
        for sparse in [false,true] {
            let removed = regions(&empty,&old,(8,8),false,sparse);
            assert_eq!(removed, vec![PixelRect::full(8,8)]);
            assert_eq!(removed, regions(&old,&empty,(8,8),false,sparse));
        }
    }
}
