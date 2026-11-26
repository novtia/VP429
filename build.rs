use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Windows defaults to a 1 MB main-thread stack, which is easily exceeded by
        // unoptimized debug builds of GPUI element trees. Match macOS/Linux (8 MB).
        println!("cargo:rustc-link-arg=/STACK:8388608");
    }

    #[cfg(windows)]
    {
        let assets_dir = Path::new("assets");
        let _ = fs::create_dir_all(assets_dir);
        let ico_path = assets_dir.join("app.ico");
        generate_swiss_v_reroute_ico(&ico_path);

        let is_release = std::env::var("PROFILE").as_deref() == Ok("release");
        let exec_level = if is_release {
            "requireAdministrator"
        } else {
            "asInvoker"
        };
        let manifest = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0" xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
    <asmv3:application>
        <asmv3:windowsSettings>
            <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
            <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
        </asmv3:windowsSettings>
    </asmv3:application>
    <dependency>
        <dependentAssembly>
            <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" />
        </dependentAssembly>
    </dependency>
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="{exec_level}" uiAccess="false" />
            </requestedPrivileges>
        </security>
    </trustInfo>
</assembly>"#
        );

        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.set("ProductName", "VP429");
        res.set("FileDescription", "VP429 — Rust + GPUI VPN");
        res.set_manifest(&manifest);
        res.compile().expect("Failed to compile Windows resources");
    }
}

/// Generates a multi-resolution anti-aliased Windows .ico for Icon 02 (Swiss V-Reroute).
#[cfg(windows)]
fn generate_swiss_v_reroute_ico(path: &Path) {
    let sizes: &[u32] = &[16, 24, 32, 48, 64, 128];
    let mut images: Vec<(u32, Vec<u8>)> = Vec::new();
    for &sz in sizes {
        images.push((sz, render_bmp_dib(sz)));
    }

    let mut out = Vec::new();
    // ICONDIR header
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type = 1 (ICO)
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());

    let header_size = 6 + 16 * images.len();
    let mut offset = header_size as u32;

    for (sz, data) in &images {
        let dim = if *sz >= 256 { 0u8 } else { *sz as u8 };
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // color count
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bit count
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }

    for (_, data) in images {
        out.extend_from_slice(&data);
    }

    let _ = fs::write(path, out);
}

#[cfg(windows)]
fn render_bmp_dib(size: u32) -> Vec<u8> {
    let mut dib = Vec::new();
    // BITMAPINFOHEADER (40 bytes)
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(size as i32).to_le_bytes());
    dib.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // height * 2 for XOR + AND mask
    dib.extend_from_slice(&1u16.to_le_bytes()); // planes
    dib.extend_from_slice(&32u16.to_le_bytes()); // 32bpp BGRA
    dib.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    // Bottom-up BGRA pixels with 4x4 supersampling
    let ss = 4usize;
    for y in (0..size).rev() {
        for x in 0..size {
            let mut r_acc = 0.0f32;
            let mut g_acc = 0.0f32;
            let mut b_acc = 0.0f32;
            let mut a_acc = 0.0f32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let u = ((x as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32) * 128.0;
                    let v = ((y as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32) * 128.0;
                    let (r, g, b, a) = sample_icon_02(u, v);
                    r_acc += r * a;
                    g_acc += g * a;
                    b_acc += b * a;
                    a_acc += a;
                }
            }
            let samples = (ss * ss) as f32;
            let a = a_acc / samples;
            let (r, g, b) = if a_acc > 1e-4 {
                (r_acc / a_acc, g_acc / a_acc, b_acc / a_acc)
            } else {
                (0.0, 0.0, 0.0)
            };
            dib.push((b * 255.0).clamp(0.0, 255.0) as u8);
            dib.push((g * 255.0).clamp(0.0, 255.0) as u8);
            dib.push((r * 255.0).clamp(0.0, 255.0) as u8);
            dib.push((a * 255.0).clamp(0.0, 255.0) as u8);
        }
    }

    // AND mask (all 0s, alpha channel controls transparency)
    let row_bytes = ((size + 31) / 32) * 4;
    dib.resize(dib.len() + (row_bytes * size) as usize, 0);
    dib
}

#[cfg(windows)]
fn dist_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

#[cfg(windows)]
fn sd_rounded_rect(px: f32, py: f32, half: f32, radius: f32) -> f32 {
    let qx = (px - 64.0).abs() - (half - radius);
    let qy = (py - 64.0).abs() - (half - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let inside = qx.max(qy).min(0.0);
    outside + inside - radius
}

/// Icon 02: Swiss V-Reroute (#ebeae4 chassis, #bdbab0 border, #191918 left V, #e84c1e right reroute)
#[cfg(windows)]
fn sample_icon_02(x: f32, y: f32) -> (f32, f32, f32, f32) {
    let d_box = sd_rounded_rect(x, y, 58.0, 24.0);
    if d_box > 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    // Base warm stone #ebeae4 or border #bdbab0
    let (mut r, mut g, mut b) = if d_box > -3.5 {
        (0xbd as f32 / 255.0, 0xba as f32 / 255.0, 0xb0 as f32 / 255.0)
    } else {
        (0xeb as f32 / 255.0, 0xea as f32 / 255.0, 0xe4 as f32 / 255.0)
    };

    // Dark charcoal V-left stroke: (32,34) -> (62,96)
    let d_dark = dist_segment(x, y, 32.0, 34.0, 62.0, 96.0);
    if d_dark <= 8.0 {
        r = 0x19 as f32 / 255.0;
        g = 0x19 as f32 / 255.0;
        b = 0x18 as f32 / 255.0;
    }

    // Braun signal orange reroute stroke: (62,96) -> (96,34)
    let d_orange = dist_segment(x, y, 62.0, 96.0, 96.0, 34.0);
    if d_orange <= 8.0 {
        r = 0xe8 as f32 / 255.0;
        g = 0x4c as f32 / 255.0;
        b = 0x1e as f32 / 255.0;
    }

    (r, g, b, 1.0)
}
