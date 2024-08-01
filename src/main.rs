use anyhow::Result;
use clap::{AppSettings, Clap};
use gb_reader::{
    board::CubicStyleBoard,
    mbc::new_mbc_reader,
    mbc::new_repl_mbc_reader,
    rom::MbcType
};
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::str;
use std::{thread, time::Duration};

#[derive(Clap)]
#[clap(version = "0.1.0", author = "mjhd <mjhd.devlion@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    Read(Read),
    ReadRam(ReadRam),
    WriteRam(WriteRam),
}

#[derive(Clap)]
struct Read {
    #[clap(short, long)]
    output: String,

    #[clap(short, long)]
    repl: bool,
}

#[derive(Clap)]
struct ReadRam {  // Options for ReadRam subcommand
    #[clap(short, long)]
    output: String,

    #[clap(short, long)]
    repl: bool,
}

#[derive(Clap)]
struct WriteRam {  // Options for ReadRam subcommand
    #[clap(short, long)]
    input: String,

    #[clap(short, long)]
    repl: bool,
}

fn read_rom(output: String, repl: bool) -> Result<()> {
    println!("[0/4] 拡張ボードの初期化中...");
    let mut board = CubicStyleBoard::new()?;

    println!("[1/4] ROMヘッダの解析中...");
    let (mut reader, header) = if repl {
        new_repl_mbc_reader(&mut board)?
    } else {
        new_mbc_reader(&mut board)?
    };

    println!(
        "タイトル: {}, MBC: {:?}, ROMサイズ: {}",
        str::from_utf8(&header.title[..]).unwrap_or("ERR"),
        header.mbc_type,
        HumanBytes(header.rom_size as u64)
    );

    println!("[2/4] 出力ファイルの作成中...");
    let mut file = File::create(output)?;

    let total = reader.size();

    let reading = ProgressBar::new(total as u64);
    reading.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}({eta})] {msg} [{bar:.cyan/blue}] {bytes}/{total_bytes}")
            .progress_chars("#>-"),
    );

    println!("[3/4] ROM読み込み中...");

    loop {
        let mut buffer = [0; 0x0100];

        let size = reader.read(&mut buffer)?;

        if size == 0 {
            break;
        }

        file.write(&buffer[0..size])?;

        reading.inc(size as u64);
        reading.set_message(&reader.status());
    }

    println!("[4/4] 仕上げ中...");
    file.flush()?;

    println!("完了！");
    reading.finish_and_clear();

    Ok(())
}

fn enable_ram(board: &mut CubicStyleBoard, mbc_type: MbcType) {
    board.enable_ram(mbc_type);
}

fn disable_ram(board: &mut CubicStyleBoard, mbc_type: MbcType) {
    board.disable_ram(mbc_type);
}

fn set_addr(board: &mut CubicStyleBoard, mbc_type: u16) {
    board.set_addr(mbc_type);
}

fn read_byte(board: &mut CubicStyleBoard) -> u8 {
    return board.read_byte().unwrap();
}

fn read_ram(output: String, repl: bool) -> Result<()> {
    println!("[0/] Initializing board...");
    let mut board = CubicStyleBoard::new()?;
    println!("[0/6] Board initialized");

    println!("[1/6] ROMヘッダの解析中...");

    let (mut reader, header) = if repl {
        new_repl_mbc_reader(&mut board)?
    } else {
        new_mbc_reader(&mut board)?
    };

    println!("RAM size: {:?}", &header.ram_size);

    println!("ROM title: {:?}", std::str::from_utf8(&header.title).unwrap_or("ERR"));
    println!("MBC type: {:?}", header.mbc_type);

    println!("[2/6] Enabling RAM...");
    reader.enable_ram(header.mbc_type);
    println!("[2/6] RAM enabled");

    println!("[3/6] Creating output file...");
    let mut file = File::create(output)?;
    println!("[3/6] Output file created");

    // Determine the number of RAM banks based on RAM size
    let bank_size = 0x2000; // 8KB per bank
    let num_banks = header.ram_size / bank_size;
    println!("RAM size: {}", header.ram_size);
    println!("Bank size: {}", bank_size);
    println!("Number of RAM banks: {}", num_banks);

    println!("[4/6] Reading RAM...");
    for bank in 0..num_banks {
        println!("Switching to RAM bank {}", bank);
        // Switch to the current bank if the MBC type supports it
        match header.mbc_type {
            MbcType::Mbc1 | MbcType::Mbc1Ram | MbcType::Mbc1RamBattery => {
                reader.select_ram_bank(bank as u8);
            }
            MbcType::Mbc3 | MbcType::Mbc3Ram | MbcType::Mbc3RamBattery => {
                reader.select_ram_bank(bank as u8);
            }
            MbcType::Mbc5 | MbcType::Mbc5Ram | MbcType::Mbc5RamBattery => {
                reader.select_ram_bank(bank as u8);
            }
            _ => {
                // If the MBC type does not support multiple RAM banks, continue as is
            }
        }

        for addr in 0xA000..=0xBFFF {
            reader.set_addr(addr);
            let data = reader.read_byte()?;
            // Only print the first few bytes for debugging
            if addr < 0xA010 {
                println!("Address: {:04X}, Data: {:02X}", addr, data);
            }
            file.write_all(&[data])?;
        }
    }

    println!("[5/6] Disabling RAM...");
    reader.disable_ram(header.mbc_type);
    println!("[5/6] RAM disabled");

    println!("[6/6] Finalizing...");
    file.flush()?;

    println!("Completed!");
    Ok(())
}

fn write_ram(input: String, repl: bool) -> Result<()> {
    println!("[0/7] Initializing board...");
    let mut board = CubicStyleBoard::new()?;
    println!("[0/7] Board initialized");

    println!("[1/7] ROMヘッダの解析中...");

    let (mut reader, header) = if repl {
        new_repl_mbc_reader(&mut board)?
    } else {
        new_mbc_reader(&mut board)?
    };

    println!("RAM size: {}", header.ram_size);

    println!("ROM title: {:?}", std::str::from_utf8(&header.title).unwrap_or("ERR"));
    println!("MBC type: {:?}", header.mbc_type);

    println!("[2/7] Applying MBC2 fix...");
    reader.set_addr(0x0134);
    reader.read_byte()?;
    println!("[2/7] MBC2 fix applied");

    // Check if cartridge has RAM
    if header.ram_size > 0 {
        match header.mbc_type {
            MbcType::Mbc1 | MbcType::Mbc1Ram | MbcType::Mbc1RamBattery => {
                println!("Setting RAM mode for MBC1...");
                reader.set_addr(0x6000);
                reader.write_byte(1)?;
            }
            _ => {}
        }

        println!("[3/7] Enabling RAM...");
        reader.set_addr(0x0000);
        reader.write_byte(0x0A)?;
        println!("[3/7] RAM enabled");

        println!("[4/7] Opening input file...");
        let mut file = File::open(input)?;
        println!("[4/7] Input file opened");

        // Determine the number of RAM banks based on RAM size
        let bank_size = 8 * 1024; // 8KB per bank
        let num_banks = header.ram_size / bank_size;
        println!("Number of RAM banks: {}", num_banks);

        println!("[5/7] Writing to RAM...");
        let mut buffer = [0; 0x2000]; // 8KB buffer
        for bank in 0..num_banks {
            println!("Switching to RAM bank {}", bank);
            reader.select_ram_bank(bank as u8)?;

            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break; // End of file reached
            }

            for (i, &data) in buffer.iter().enumerate().take(bytes_read) {
                let addr = 0xA000 + i as u16;
                reader.set_addr(addr);
                thread::sleep(Duration::from_micros(1)); // Add a small delay
                reader.write_byte(data)?;
                thread::sleep(Duration::from_micros(1)); // Add a small delay
                if i < 16 || i >= bytes_read - 16 {
                    println!("Writing to Address: {:04X}, Data: {:02X}", addr, data); // Debugging statement
                }
            }

            if bytes_read < buffer.len() {
                for i in bytes_read..buffer.len() {
                    let addr = 0xA000 + i as u16;
                    reader.set_addr(addr);
                    thread::sleep(Duration::from_micros(1)); // Add a small delay
                    reader.write_byte(0xFF)?;
                    thread::sleep(Duration::from_micros(1)); // Add a small delay
                    if i < bytes_read + 16 {
                        println!("Filling Address: {:04X}, Data: 0xFF", addr); // Debugging statement
                    }
                }
            }
        }

        println!("[6/7] Disabling RAM...");
        reader.set_addr(0x0000);
        reader.write_byte(0x00)?;
        println!("[6/7] RAM disabled");

        println!("[7/7] Finalizing...");
        println!("Completed!");
        Ok(())
    } else {
        println!("No RAM present in cartridge.");
        Ok(())
    }
}

// Helper function to add a small delay
fn delay_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

fn file_test(input: String, repl: bool) -> Result<()> {
    println!("Opening file...{}", input);
    File::open(input).unwrap();
    Ok(())
}

fn main() {
    println!("Parsing command line options...");
    let opts: Opts = Opts::parse();
    println!("Command line options parsed");

    let result = match opts.subcmd {
        SubCommand::Read(t) => {
            println!("Executing read_rom function...");
            read_rom(t.output, t.repl)
        },
        SubCommand::ReadRam(t) => {
            println!("Executing read_ram function...");
            read_ram(t.output, t.repl)
        },
        SubCommand::WriteRam(t) => {
            println!("Executing write_ram function...");
            write_ram(t.input, t.repl)
        },
    };

    println!("Execution result: {:?}", result);
    result.unwrap();
}
