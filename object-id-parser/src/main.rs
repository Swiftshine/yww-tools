use anyhow::{Result, ensure};
use byteorder::{BigEndian, ReadBytesExt};
use std::{
    env, fs,
    io::{BufRead, Cursor},
};

// USv32 address
const TABLE_ADDRESS_START: usize = 0x102A21BC;

#[derive(Debug)]
struct ObjectID {
    id: u32,
    name: String,
}

#[derive(Debug)]
struct Conversion {
    global: ObjectID,
    local: ObjectID,
}

#[derive(Debug)]
struct Category {
    name: String,
    objects: Vec<Conversion>,
}

// specifically for the 0x1XXXXXXX range
const fn address_to_elf_offset(address: usize) -> usize {
    const REF_ADDR: usize = 0x102a21c0;
    const REF_FILE_OFFSET: usize = 0x2A2980;

    let offset = REF_ADDR - REF_FILE_OFFSET;

    address - offset
}

struct Reader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(bytes),
        }
    }

    fn set_offset(&mut self, offset: usize) {
        self.cursor.set_position(offset as u64);
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(self.cursor.read_u32::<BigEndian>()?)
    }

    fn read_string(&mut self) -> Result<String> {
        let mut bytes = Vec::new();
        self.cursor.read_until(0, &mut bytes)?;

        // remove null terminator
        if bytes.last() == Some(&0) {
            bytes.pop();
        }

        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn read_object_id(&mut self) -> Result<ObjectID> {
        let id = self.read_u32()?;

        let name = {
            let string_pointer = self.read_u32()?;
            let string_offset = address_to_elf_offset(string_pointer as usize);
            let pos = self.cursor.position();

            self.cursor.set_position(string_offset as u64);

            let name = self.read_string()?;

            self.cursor.set_position(pos);

            name
        };

        Ok(ObjectID { id, name })
    }

    fn read_conversion(&mut self) -> Result<Conversion> {
        let global = self.read_object_id()?;
        let local = self.read_object_id()?;

        Ok(Conversion { global, local })
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    ensure!(args.len() == 2);

    let elf = fs::read(&args[1])?;

    let mut reader = Reader::new(&elf);
    reader.set_offset(address_to_elf_offset(TABLE_ADDRESS_START));

    // category, count
    let map = [
        ("Player", 1),
        ("PlEgg", 1),
        ("Enemy", 130), // the game's code specifies 128 but *really* the list extends for 2 more than that
        ("Gimmick", 455),
        ("Item", 18),
        ("Terrain", 41),
    ];

    let mut categories = Vec::new();

    for (category, count) in map {
        let mut objects = Vec::new();

        for _ in 0..count {
            objects.push(reader.read_conversion()?);
        }

        categories.push(Category {
            name: category.to_string(),
            objects,
        });
    }

    // generate object id enum

    println!("ENUM_CLASS(ObjectID,");

    for category in &categories {
        let max_len = category
            .objects
            .iter()
            .map(|c| c.global.name.len())
            .max()
            .unwrap_or(0);

        for conv in &category.objects {
            let name = &conv.global.name;

            println!(
                "    {:<width$} = {},",
                name,
                conv.global.id,
                width = max_len
            );
        }

        println!();
    }

    println!(");");

    // generate local id enums

    for category in &categories {
        if category.objects.len() <= 1 {
            continue;
        }

        println!("ENUM_CLASS({}ID,", category.name);

        let max_len = category
            .objects
            .iter()
            .map(|c| c.local.name.len())
            .max()
            .unwrap_or(0);

        for conv in &category.objects {
            let name = &conv.local.name;

            println!(
                "    {:<width$} = {},",
                name,
                conv.local.id,
                width = max_len
            );
        }

        println!(");");
        println!();
    }

    Ok(())
}
