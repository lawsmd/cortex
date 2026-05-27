use warpui::prelude::ColorU;

pub struct NerdIcon {
    pub glyph: char,
    pub color: ColorU,
}

fn hex(r: u8, g: u8, b: u8) -> ColorU {
    ColorU::new(r, g, b, 255)
}

pub fn nerd_icon_for_path(
    filename: &str,
    is_dir: bool,
    is_expanded: bool,
    default_color: ColorU,
) -> NerdIcon {
    if is_dir {
        return nerd_icon_for_directory(filename, is_expanded);
    }

    if let Some(icon) = nerd_icon_for_exact_filename(filename) {
        return icon;
    }

    let ext = filename.rsplit('.').next().unwrap_or("");
    nerd_icon_for_extension(ext, default_color)
}

fn nerd_icon_for_directory(name: &str, is_expanded: bool) -> NerdIcon {
    let color = match name {
        ".git" => hex(0xF1, 0x4C, 0x28),
        "node_modules" => hex(0x89, 0xE0, 0x51),
        "src" | "lib" => hex(0xE8, 0xA3, 0x17),
        "target" | "build" | "dist" | "out" => hex(0x9B, 0x9B, 0x9B),
        "test" | "tests" | "spec" | "specs" => hex(0x3F, 0xB9, 0x50),
        "docs" | "doc" => hex(0x42, 0xA5, 0xF5),
        _ => hex(0xE8, 0xA3, 0x17),
    };

    let glyph = match name {
        ".git" => '\u{F1D3}',
        _ if is_expanded => '\u{F115}',
        _ => '\u{F114}',
    };

    NerdIcon { glyph, color }
}

fn nerd_icon_for_exact_filename(name: &str) -> Option<NerdIcon> {
    let (glyph, color) = match name {
        "Cargo.toml" | "Cargo.lock" => ('\u{E7A8}', hex(0xDE, 0xA5, 0x84)),
        "Makefile" | "makefile" | "GNUmakefile" => ('\u{E615}', hex(0x6D, 0x80, 0x86)),
        "Dockerfile" | "dockerfile" => ('\u{F308}', hex(0x38, 0x4D, 0x54)),
        "docker-compose.yml" | "docker-compose.yaml" => ('\u{F308}', hex(0x38, 0x4D, 0x54)),
        ".gitignore" | ".gitattributes" | ".gitmodules" => ('\u{F1D3}', hex(0xF1, 0x4C, 0x28)),
        ".env" | ".env.local" | ".env.example" => ('\u{F462}', hex(0xFF, 0xD6, 0x00)),
        "LICENSE" | "LICENSE.md" | "LICENSE.txt" | "LICENSE-MIT" | "LICENSE-AGPL" => {
            ('\u{F0219}', hex(0xCB, 0xCB, 0x41))
        }
        "README.md" | "README" | "README.txt" => ('\u{F48A}', hex(0x42, 0xA5, 0xF5)),
        "package.json" | "package-lock.json" => ('\u{E74E}', hex(0xF1, 0xE0, 0x5A)),
        "tsconfig.json" => ('\u{E628}', hex(0x31, 0x78, 0xC6)),
        ".eslintrc" | ".eslintrc.json" | ".eslintrc.js" => ('\u{E60C}', hex(0x47, 0x31, 0xA9)),
        "CLAUDE.md" => ('\u{F10D3}', hex(0xD9, 0x7D, 0x48)),
        _ => return None,
    };
    Some(NerdIcon { glyph, color })
}

fn nerd_icon_for_extension(ext: &str, default_color: ColorU) -> NerdIcon {
    let (glyph, color) = match ext {
        "rs" => ('\u{E7A8}', hex(0xDE, 0xA5, 0x84)),
        "py" | "pyi" | "pyw" => ('\u{E73C}', hex(0x35, 0x72, 0xA5)),
        "js" | "mjs" | "cjs" => ('\u{E74E}', hex(0xF1, 0xE0, 0x5A)),
        "ts" | "mts" | "cts" => ('\u{E628}', hex(0x31, 0x78, 0xC6)),
        "tsx" => ('\u{E7BA}', hex(0x31, 0x78, 0xC6)),
        "jsx" => ('\u{E7BA}', hex(0xF1, 0xE0, 0x5A)),
        "json" | "jsonc" | "json5" => ('\u{E60B}', hex(0xCB, 0xCB, 0x41)),
        "md" | "mdx" => ('\u{E73E}', hex(0xDD, 0xDD, 0xDD)),
        "toml" => ('\u{E615}', hex(0x9C, 0x42, 0x21)),
        "yaml" | "yml" => ('\u{E6A8}', hex(0xCB, 0x17, 0x1E)),
        "go" => ('\u{E626}', hex(0x00, 0xAD, 0xD8)),
        "c" => ('\u{E61E}', hex(0x55, 0x55, 0xDD)),
        "h" => ('\u{E61E}', hex(0xA0, 0x74, 0xC4)),
        "cpp" | "cc" | "cxx" | "c++" => ('\u{E61D}', hex(0xF3, 0x4B, 0x7D)),
        "hpp" | "hh" | "hxx" | "h++" => ('\u{E61D}', hex(0xA0, 0x74, 0xC4)),
        "sh" | "bash" | "zsh" | "fish" => ('\u{F489}', hex(0x89, 0xE0, 0x51)),
        "ps1" | "psm1" | "psd1" => ('\u{F489}', hex(0x01, 0x2A, 0x56)),
        "bat" | "cmd" => ('\u{F489}', hex(0xC1, 0xF1, 0x2E)),
        "html" | "htm" => ('\u{E736}', hex(0xE3, 0x4C, 0x26)),
        "css" => ('\u{E749}', hex(0x56, 0x3D, 0x7C)),
        "scss" | "sass" => ('\u{E749}', hex(0xCD, 0x66, 0x99)),
        "less" => ('\u{E749}', hex(0x1D, 0x36, 0x5D)),
        "svg" => ('\u{F0721}', hex(0xFF, 0xB1, 0x3B)),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" => {
            ('\u{F1C5}', hex(0xA0, 0x74, 0xC4))
        }
        "lock" => ('\u{F023}', hex(0x9B, 0x9B, 0x9B)),
        "xml" => ('\u{F05C0}', hex(0xE3, 0x7A, 0x33)),
        "sql" => ('\u{E706}', hex(0xDA, 0xDA, 0x33)),
        "rb" => ('\u{E739}', hex(0xCC, 0x34, 0x2D)),
        "java" => ('\u{E738}', hex(0xCC, 0x32, 0x35)),
        "kt" | "kts" => ('\u{E634}', hex(0x7F, 0x52, 0xFF)),
        "swift" => ('\u{E755}', hex(0xFF, 0xAC, 0x45)),
        "php" => ('\u{E73D}', hex(0x47, 0x78, 0x99)),
        "lua" => ('\u{E620}', hex(0x00, 0x00, 0x80)),
        "vim" | "vimrc" => ('\u{E62B}', hex(0x01, 0x99, 0x33)),
        "zig" => ('\u{E6A9}', hex(0xF6, 0x9D, 0x50)),
        "nix" => ('\u{F313}', hex(0x7E, 0xBF, 0xFC)),
        "txt" | "text" => ('\u{F15C}', hex(0x9B, 0x9B, 0x9B)),
        "log" => ('\u{F18D}', hex(0xAF, 0xAF, 0xAF)),
        "conf" | "cfg" | "ini" => ('\u{E615}', hex(0x6D, 0x80, 0x86)),
        "env" => ('\u{F462}', hex(0xFF, 0xD6, 0x00)),
        "tf" | "hcl" => ('\u{E69A}', hex(0x5C, 0x4E, 0xE5)),
        "wasm" => ('\u{E6A1}', hex(0x65, 0x4F, 0xF0)),
        "graphql" | "gql" => ('\u{E662}', hex(0xE5, 0x35, 0xAB)),
        "proto" => ('\u{E6A8}', hex(0x6D, 0x80, 0x86)),
        "csv" | "tsv" => ('\u{F1C3}', hex(0x89, 0xE0, 0x51)),
        "pdf" => ('\u{F1C1}', hex(0xF4, 0x00, 0x00)),
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => {
            ('\u{F1C6}', hex(0xAF, 0xAF, 0xAF))
        }
        "exe" | "dll" | "so" | "dylib" => ('\u{F013}', hex(0x9B, 0x9B, 0x9B)),
        _ => ('\u{F15B}', default_color),
    };
    NerdIcon { glyph, color }
}
