//! 构建时程序化生成托盘/程序图标(纯 Rust,无外部依赖)。
//!
//! 设计:圆角矩形背景 + 蓝色垂直渐变 + 白色 "C" 字形
//! (CapsLock 首字母)+ 底部输入光标下划线。输出多尺寸 ICO
//! (16/32/48),写入 OUT_DIR/icon.ico。

use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = build_ico(&[16, 32, 48]);
    let ico_path = out.join("icon.ico");
    std::fs::write(&ico_path, &ico).expect("write icon.ico");

    // 将图标嵌入 exe 资源(任务栏/快捷方式/资源管理器显示),失败不致命
    let mut res = winres::WindowsResource::new();
    if res
        .set_icon(ico_path.to_str().unwrap_or_default())
        .compile()
        .is_err()
    {
        eprintln!("warning: 未能将图标嵌入 exe 资源");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

/// "C" 字形 5x7 点阵
const C_GLYPH: [&[u8; 5]; 7] = [
    b" ### ",
    b"#    ",
    b"#    ",
    b"#    ",
    b"#    ",
    b"#    ",
    b" ### ",
];

/// 组装多尺寸 ICO 文件
fn build_ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|s| build_image(*s)).collect();

    let mut out = Vec::new();
    // ICONDIR:reserved(2) type=1 icon(2) count(2)
    out.extend_from_slice(&[0, 0, 1, 0, sizes.len() as u8, 0]);

    // ICONDIRENTRY x N
    let mut offset: u32 = 6 + 16 * sizes.len() as u32;
    for (i, img) in images.iter().enumerate() {
        let s = sizes[i];
        out.push(s as u8); // width
        out.push(s as u8); // height
        out.extend_from_slice(&[0, 0]); // colors, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bitcount
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }

    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

/// 生成单尺寸图标数据:DIB 头 + 32bpp BGRA 像素(自底向上)+ AND 掩码
fn build_image(size: u32) -> Vec<u8> {
    let xor_len = (size * size * 4) as usize;
    let and_row = (size.div_ceil(32) * 4) as usize; // AND 掩码每行字节数(4 对齐)
    let and_len = and_row * size as usize;

    let mut out = Vec::with_capacity(40 + xor_len + and_len);

    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&size.to_le_bytes()); // biWidth
    out.extend_from_slice(&(size * 2).to_le_bytes()); // biHeight(含 AND 掩码)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression
    out.extend_from_slice(&((xor_len + and_len) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // 其余字段

    // XOR 像素(自底向上行序)
    let glyph_scale = ((size as f32 * 0.72 / 7.0).round() as u32).max(1);
    let glyph_w = 5 * glyph_scale;
    let glyph_h = 7 * glyph_scale;
    let gx = (size.saturating_sub(glyph_w)) / 2;
    let gy = (size.saturating_sub(glyph_h)) / 2;

    for row in 0..size {
        let y = size - 1 - row; // 自底向上
        for x in 0..size {
            out.extend_from_slice(&pixel(x, y, size, gx, gy, glyph_scale));
        }
    }

    // AND 掩码:全 0(透明由 alpha 通道负责)
    out.extend(std::iter::repeat_n(0u8, and_len));
    out
}

/// 计算单个像素 BGRA
fn pixel(x: u32, y: u32, size: u32, gx: u32, gy: u32, scale: u32) -> [u8; 4] {
    let s = size as f32;
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);

    // 圆角矩形:四角半径 r
    let r = s * 0.18;
    let cx = fx.clamp(r, s - r);
    let cy = fy.clamp(r, s - r);
    let (dx, dy) = (fx - cx, fy - cy);
    if dx * dx + dy * dy > r * r {
        return [0, 0, 0, 0]; // 圆角外:透明
    }

    // 垂直渐变:顶部亮蓝 → 底部深蓝
    let t = fy / s;
    let r_ch = lerp(0x2D, 0x0F, t);
    let g_ch = lerp(0x9E, 0x63, t);
    let b_ch = lerp(0xFF, 0xE5, t);

    // "C" 字形:白色
    if x >= gx && x < gx + 5 * scale && y >= gy && y < gy + 7 * scale {
        let glyph_x = ((x - gx) / scale) as usize;
        let glyph_y = ((y - gy) / scale) as usize;
        if C_GLYPH[glyph_y][glyph_x] == b'#' {
            return uw(255);
        }
    }

    // 底部输入光标下划线:白色(仅大尺寸可见)
    if size >= 24 {
        let bar_y = (s * 0.84) as u32;
        let bar_h = (s * 0.045).max(1.5) as u32;
        let bar_x0 = (s * 0.28) as u32;
        let bar_x1 = (s * 0.72) as u32;
        if y >= bar_y && y < bar_y + bar_h && x >= bar_x0 && x <= bar_x1 {
            return uw(235);
        }
    }

    [b_ch, g_ch, r_ch, 255]
}

#[inline]
fn uw(tint: u8) -> [u8; 4] {
    [tint, tint, tint, 255]
}

#[inline]
fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}