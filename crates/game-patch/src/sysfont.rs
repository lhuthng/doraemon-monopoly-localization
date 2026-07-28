use crate::Result;

pub const ORIGINAL_GLYPHS: usize = 640;
pub const EXTENDED_GLYPHS: usize = 1_920;
pub const VIETNAMESE_SLOTS: usize = 256;
pub const VIETNAMESE_VARIANTS: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Glyph {
    pub width: u8,
    pub height: u8,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Font {
    pub header: Vec<u8>,
    pub glyphs: Vec<Glyph>,
}

fn u16(data: &[u8], offset: usize) -> Result<usize> {
    let bytes: [u8; 2] = data
        .get(offset..offset + 2)
        .ok_or("truncated sysfont")?
        .try_into()
        .unwrap();
    Ok(u16::from_le_bytes(bytes) as usize)
}
fn u32(data: &[u8], offset: usize) -> Result<usize> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or("truncated sysfont")?
        .try_into()
        .unwrap();
    Ok(u32::from_le_bytes(bytes) as usize)
}

pub fn parse(data: &[u8]) -> Result<Font> {
    let count = u16(data, 0)?;
    if count == 0 || count % 128 != 0 || 2 + count * 4 > data.len() {
        return Err(format!("invalid sysfont glyph count {count}"));
    }
    let first = u32(data, 2)?;
    if first < 2 + count * 4 || first > data.len() {
        return Err("invalid sysfont header".into());
    }
    let mut glyphs = Vec::with_capacity(count);
    for index in 0..count {
        let offset = u32(data, 2 + index * 4)?;
        let width = *data
            .get(offset)
            .ok_or_else(|| format!("glyph {index} is outside sysfont"))?;
        let height = *data
            .get(offset + 1)
            .ok_or_else(|| format!("glyph {index} is outside sysfont"))?;
        if width == 0 || height == 0 || width > 96 || height > 96 {
            return Err(format!(
                "glyph {index} has invalid dimensions {width}x{height}"
            ));
        }
        let length = width as usize * height as usize;
        glyphs.push(Glyph {
            width,
            height,
            pixels: data
                .get(offset + 2..offset + 2 + length)
                .ok_or_else(|| format!("glyph {index} pixels are truncated"))?
                .to_vec(),
        });
    }
    Ok(Font {
        header: data[2 + count * 4..first].to_vec(),
        glyphs,
    })
}

pub fn rebuild(font: &Font) -> Result<Vec<u8>> {
    let count = font.glyphs.len();
    let count16 = u16::try_from(count).map_err(|_| "too many sysfont glyphs".to_string())?;
    let first = 2 + count * 4 + font.header.len();
    let mut output = vec![0_u8; first];
    output[..2].copy_from_slice(&count16.to_le_bytes());
    output[2 + count * 4..first].copy_from_slice(&font.header);
    for (index, glyph) in font.glyphs.iter().enumerate() {
        if glyph.width == 0
            || glyph.height == 0
            || glyph.pixels.len() != glyph.width as usize * glyph.height as usize
        {
            return Err(format!("invalid glyph {index}"));
        }
        let offset =
            u32::try_from(output.len()).map_err(|_| "sysfont exceeds 4 GiB".to_string())?;
        output[2 + index * 4..6 + index * 4].copy_from_slice(&offset.to_le_bytes());
        output.push(glyph.width);
        output.push(glyph.height);
        output.extend_from_slice(&glyph.pixels);
    }
    if parse(&output)?.glyphs != font.glyphs {
        return Err("rebuilt sysfont verification failed".into());
    }
    Ok(output)
}

fn ascii_base(character: char) -> u8 {
    match character {
        'Đ' => b'D',
        'đ' => b'd',
        _ => character as u32 as u8,
    }
}

fn contains(set: &str, character: char) -> bool {
    set.chars().any(|candidate| candidate == character)
}

fn generated_glyph(mut base: Glyph, character: char) -> Glyph {
    let width = base.width as i32;
    let height = base.height as i32;
    let center = width / 2;
    let mut set = |x: i32, y: i32| {
        if x >= 0 && x < width && y >= 0 && y < height {
            base.pixels[(y * width + x) as usize] = 0;
        }
    };
    if character == 'Đ' || character == 'đ' {
        let y = std::cmp::max(1, height / 2);
        for x in 0..width {
            set(x, y);
        }
        return base;
    }
    let mut row = 0;
    if contains("ăằắẳẵặĂẰẮẲẴẶ", character) {
        set(center - 1, row);
        set(center, row + 1);
        set(center + 1, row);
        row += 2;
    } else if contains("âầấẩẫậêềếểễệôồốổỗộÂẦẤẨẪẬÊỀẾỂỄỆÔỒỐỔỖỘ", character)
    {
        set(center - 1, row + 1);
        set(center, row);
        set(center + 1, row + 1);
        row += 2;
    } else if contains("ơờớởỡợưừứửữựƠỜỚỞỠỢƯỪỨỬỮỰ", character)
    {
        set(width - 2, std::cmp::min(2, row));
        set(width - 1, std::cmp::min(1, row));
        row += 1;
    }
    if contains("àằầèềìòồờùừỳÀẰẦÈỀÌÒỒỜÙỪỲ", character) {
        set(center - 1, row);
        set(center, row + 1);
    } else if contains("áắấéếíóốớúứýÁẮẤÉẾÍÓỐỚÚỨÝ", character) {
        set(center, row + 1);
        set(center + 1, row);
    } else if contains("ảẳẩẻểỉỏổởủửỷẢẲẨẺỂỈỎỔỞỦỬỶ", character)
    {
        set(center, row);
        set(center + 1, row);
        set(center, row + 1);
    } else if contains("ãẵẫẽễĩõỗỡũữỹÃẴẪẼỄĨÕỖỠŨỮỸ", character)
    {
        set(center - 2, row + 1);
        set(center - 1, row);
        set(center, row + 1);
        set(center + 1, row);
    } else if contains("ạặậẹệịọộợụựỵẠẶẬẸỆỊỌỘỢỤỰỴ", character)
    {
        set(center, height - 1);
    }
    base
}

// Build a deterministic authored bank from the proportional ASCII glyphs.
// Release payloads may replace individual Vietnamese records with hand-edited
// artwork, but the standalone command never needs an original target archive.
pub fn extend(data: &[u8]) -> Result<Vec<u8>> {
    let mut font = parse(data)?;
    if font.glyphs.len() == EXTENDED_GLYPHS {
        return Ok(data.to_vec());
    }
    if font.glyphs.len() != ORIGINAL_GLYPHS {
        return Err(format!(
            "Vietnamese extension requires {ORIGINAL_GLYPHS} glyphs; found {}",
            font.glyphs.len()
        ));
    }
    let characters = vietnamese_characters();
    for variant in 0..VIETNAMESE_VARIANTS {
        for slot in 0..VIETNAMESE_SLOTS {
            let character = characters.get(slot).copied().unwrap_or(' ');
            let base_index = variant * 128 + ascii_base(deaccent(character)) as usize;
            let base = font.glyphs[base_index].clone();
            let glyph = if variant == 0 && slot < characters.len() {
                generated_glyph(base, character)
            } else {
                Glyph {
                    width: base.width,
                    height: base.height,
                    pixels: vec![0xff; base.pixels.len()],
                }
            };
            font.glyphs.push(glyph);
        }
    }
    rebuild(&font)
}

fn deaccent(character: char) -> char {
    match character {
        'đ' => 'd',
        'Đ' => 'D',
        'ă' | 'â' | 'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'ầ' | 'ấ' | 'ẩ'
        | 'ẫ' | 'ậ' => 'a',
        'Ă' | 'Â' | 'À' | 'Á' | 'Ả' | 'Ã' | 'Ạ' | 'Ằ' | 'Ắ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Ầ' | 'Ấ' | 'Ẩ'
        | 'Ẫ' | 'Ậ' => 'A',
        'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => 'e',
        'È' | 'É' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ề' | 'Ế' | 'Ể' | 'Ễ' | 'Ệ' => 'E',
        'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'Ì' | 'Í' | 'Ỉ' | 'Ĩ' | 'Ị' => 'I',
        'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ơ' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ờ' | 'ớ' | 'ở'
        | 'ỡ' | 'ợ' => 'o',
        'Ò' | 'Ó' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ơ' | 'Ồ' | 'Ố' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ờ' | 'Ớ' | 'Ở'
        | 'Ỡ' | 'Ợ' => 'O',
        'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => 'u',
        'Ù' | 'Ú' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ử' | 'Ữ' | 'Ự' => 'U',
        'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        'Ỳ' | 'Ý' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'Y',
        other => other,
    }
}

pub fn vietnamese_characters() -> Vec<char> {
    "àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđÀÁẢÃẠĂẰẮẲẴẶÂẦẤẨẪẬÈÉẺẼẸÊỀẾỂỄỆÌÍỈĨỊÒÓỎÕỌÔỒỐỔỖỘƠỜỚỞỠỢÙÚỦŨỤƯỪỨỬỮỰỲÝỶỸỴĐ".chars().collect()
}

pub fn encoded_bytes(character: char) -> Option<[u8; 2]> {
    vietnamese_characters()
        .iter()
        .position(|candidate| *candidate == character)
        .map(|slot| [0xcc + (slot / 128) as u8, (slot % 128) as u8])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_is_stable() {
        assert_eq!(vietnamese_characters().len(), 134);
        assert_eq!(encoded_bytes('à'), Some([0xcc, 0]));
        assert_eq!(encoded_bytes('Đ'), Some([0xcd, 5]));
    }
}
