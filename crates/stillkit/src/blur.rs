//! Damage-sized CPU filters. Storage is allocated only when a scene uses blur.
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, sync::Arc};
use anyhow::Result;
use crate::{DisplayList, Image, Op, PixelRect, Rect};
use crate::platform::Renderer;

pub(crate) fn reach(sigma: f32) -> f32 { (sigma.clamp(0.0, 64.0) * 3.0).ceil() }

struct Entry { source: Vec<Op>, rect: Rect, sigma: f32, image: Arc<Image> }
thread_local! { static CACHE: RefCell<VecDeque<Entry>> = const { RefCell::new(VecDeque::new()) }; }

fn raster(source: Vec<Op>, rect: Rect, sigma: f32) -> Result<Arc<Image>> {
    if let Some(image) = CACHE.with(|c| c.borrow().iter().find(|e| e.rect == rect && e.sigma == sigma && e.source == source).map(|e| e.image.clone())) { return Ok(image); }
    let ui = crate::Ui::new()?;
    let (width,height) = (rect.width() as u32, rect.height() as u32);
    let canvas = crate::platform::current::Renderer::new(&ui, width, height)?;
    let mut text = ui.text;
    let list = DisplayList { ops: source.clone(), ..Default::default() }.translated(-rect.left,-rect.top);
    canvas.draw(&mut text, &list, PixelRect::full(width,height))?;
    let mut pixels = vec![0; width as usize * height as usize * 4];
    canvas.read(PixelRect::full(width,height), |y,row| { let i=y as usize*width as usize*4; pixels[i..i+row.len()].copy_from_slice(row); })?;
    boxes(&mut pixels,width as usize,height as usize,sigma);
    let image = Arc::new(Image { width,height,pixels });
    CACHE.with(|c| {
        let mut c=c.borrow_mut();
        let bytes=image.pixels.len();
        if bytes <= 16*1024*1024 {
            while c.len() >= 16 || c.iter().map(|e|e.image.pixels.len()).sum::<usize>() + bytes > 16*1024*1024 { c.pop_front(); }
            c.push_back(Entry { source,rect,sigma,image:image.clone() });
        }
    });
    Ok(image)
}

/// Three separable sliding boxes approximate a Gaussian, with work independent of radius.
fn boxes(pixels: &mut [u8], w: usize, h: usize, sigma: f32) {
    let ideal=(4.0*sigma*sigma+1.0).sqrt();
    let mut lower=ideal.floor() as usize;
    if lower.is_multiple_of(2) { lower=lower.saturating_sub(1); }
    lower=lower.max(1);
    let n=((12.0*sigma*sigma-3.0*(lower*lower) as f32-12.0*lower as f32-9.0)/(-4.0*lower as f32-4.0)).round().clamp(0.0,3.0) as usize;
    let mut tmp=vec![0;pixels.len()];
    for pass in 0..3 {
        let r=if pass<n {lower/2} else {lower/2+1};
        for (src,dst,lines,len,stride,step) in [(true,false,h,w,w*4,4),(false,true,w,h,4,w*4)] {
            let (input,output): (&[u8],&mut [u8])=if src && !dst {(&*pixels,&mut tmp)} else {(&tmp,pixels)};
            for line in 0..lines { for ch in 0..4 {
                let base=line*stride+ch;
                let at=|i: isize| input[base+i.clamp(0,len as isize-1) as usize*step] as u32;
                let mut sum=(0..=2*r).map(|i|at(i as isize-r as isize)).sum::<u32>();
                for i in 0..len {
                    output[base+i*step]=((sum+(2*r+1) as u32/2)/(2*r+1) as u32) as u8;
                    sum=sum+at(i as isize+r as isize+1)-at(i as isize-r as isize);
                }
            } }
        }
    }
}

fn patch(source: &[Op], bounds: Rect, damage: Rect, sigma: f32) -> Result<Option<(Rect,Arc<Image>)>> {
    let output=bounds.intersect(damage);
    if output.is_empty() { return Ok(None); }
    let r=output.inflate(reach(sigma));
    let rect=Rect { left:r.left.floor(),top:r.top.floor(),right:r.right.ceil(),bottom:r.bottom.ceil() };
    let source=source.iter().filter(|op| matches!(op,Op::Clip{..}|Op::Unclip) || op.bounds().intersects(&rect)).cloned().collect();
    Ok(Some((rect,raster(source,rect,sigma)?)))
}

pub(crate) fn prepare(ops: &[Op], damage: Rect) -> Result<Cow<'_,[Op]>> {
    fn has(ops: &[Op]) -> bool { ops.iter().any(|o|match o { Op::Blurred{..}|Op::BackdropBlur{..}=>true, Op::Transformed{ops,..}|Op::Clipped{ops,..}|Op::RoundedClipped{ops,..}=>has(ops),_=>false }) }
    if !has(ops) { return Ok(Cow::Borrowed(ops)); }
    fn padding(ops: &[Op]) -> f32 { ops.iter().map(|op| match op {
        Op::Blurred {sigma,ops} => reach(*sigma)+padding(ops),
        Op::BackdropBlur {sigma,..}=>reach(*sigma),
        Op::Transformed{ops,..}|Op::Clipped{ops,..}|Op::RoundedClipped{ops,..}=>padding(ops),
        _=>0.0,
    }).sum() }
    Ok(Cow::Owned(walk(ops,&[],damage.inflate(padding(ops)))?))
}
fn walk(ops: &[Op], behind: &[Op], damage: Rect) -> Result<Vec<Op>> {
    let mut out=Vec::new();
    for op in ops {
        match op {
            Op::Blurred { sigma, ops } => {
                let bounds=op.bounds();
                let source=walk(ops,&[],bounds)?;
                if let Some((rect,image))=patch(&source,bounds,damage,*sigma)? {
                    out.push(Op::Image {rect,image,opacity:1.0,source:None});
                }
            }
            Op::BackdropBlur { rect, radius, sigma, opacity } => {
                let source: Vec<_>=behind.iter().chain(&out).cloned().collect();
                if let Some((area,image))=patch(&source,*rect,damage,*sigma)? {
                    out.push(Op::RoundedClipped {rect:*rect,radius:*radius,ops:vec![Op::Image {rect:area,image,opacity:*opacity,source:None}].into()});
                }
            }
            Op::Transformed {transform,ops} => {
                if let Some(inverse)=transform.inverse() {
                    let prefix: Vec<_>=behind.iter().chain(&out).cloned().collect();
                    let prefix=vec![Op::Transformed {transform:inverse,ops:prefix.into()}];
                    out.push(Op::Transformed {transform:*transform,ops:walk(ops,&prefix,inverse.bounds(damage))?.into()});
                }
            }
            Op::Clipped {rect,ops} | Op::RoundedClipped {rect,ops,..} => {
                let prefix: Vec<_>=behind.iter().chain(&out).cloned().collect();
                let inner=walk(ops,&prefix,damage.intersect(*rect))?.into();
                out.push(match op { Op::Clipped{..}=>Op::Clipped{rect:*rect,ops:inner}, Op::RoundedClipped{radius,..}=>Op::RoundedClipped{rect:*rect,radius:*radius,ops:inner},_=>unreachable!() });
            }
            _=>out.push(op.clone()),
        }
    }
    Ok(out)
}
