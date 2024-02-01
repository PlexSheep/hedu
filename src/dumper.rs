//! # Underlying mechanisms of how hedu works with data

use std::io::{prelude::*, Read, SeekFrom};

use anyhow::{bail, Result};
use libpt::bintols::display::humanbytes;
use libpt::log::{debug, error, trace, warn};

pub const BYTES_PER_LINE: usize = 16;
pub const LINE_SEP_HORIZ: char = '─';
pub const LINE_SEP_VERT: char = '│';
pub const CHAR_BORDER: &'static str = "|";

pub trait DataSource: Read {
    fn skip(&mut self, length: usize) -> std::io::Result<()>;
}
impl DataSource for std::io::Stdin {
    fn skip(&mut self, _length: usize) -> std::io::Result<()> {
        warn!("can't skip bytes for the stdin!");
        Ok(())
    }
}
impl DataSource for std::fs::File {
    fn skip(&mut self, length: usize) -> std::io::Result<()> {
        self.seek(SeekFrom::Current(length as i64))?;
        // returns the new position from the start, we don't need it here.
        Ok(())
    }
}

#[derive(Debug)]
pub struct Hedu {
    pub chars: bool,
    pub skip: usize,
    pub show_identical: bool,
    pub limit: usize,
    stop: bool,
    len: usize,
    data_idx: usize,
    rd_counter: usize,
    buf: [[u8; BYTES_PER_LINE]; 2],
    alt_buf: usize,
    pub display_buf: String,
    first_iter: bool,
}

impl Hedu {
    /// create a new hedu struct, initializing all kinds of configs and values needed while running
    pub fn new(chars: bool, skip: usize, show_identical: bool, limit: usize) -> Self {
        Hedu {
            chars,
            skip,
            show_identical,
            limit,
            stop: false,
            len: 0,
            data_idx: 0,
            rd_counter: 0,
            buf: [[0; BYTES_PER_LINE]; 2],
            alt_buf: 0,
            display_buf: String::new(),
            first_iter: true,
        }
    }

    /// print the display buffer to the stdout
    #[inline]
    pub fn display(&mut self) {
        println!("{}", self.display_buf);
        self.display_buf = String::new();
    }

    /// display a separator line
    #[inline]
    pub fn sep(&mut self) {
        if self.chars {
            self.display_buf += &format!("{LINE_SEP_HORIZ}").repeat(80);
        } else {
            self.display_buf += &format!("{LINE_SEP_HORIZ}").repeat(59);
        }
        self.display();
    }

    /// display a newline
    #[inline]
    pub fn newline(&mut self) {
        self.display_buf += "\n";
        self.display();
    }

    /// display the dump of a single line of data
    fn dump_a_line(&mut self) {
        self.display_buf += &format!("{:08X} {LINE_SEP_VERT} ", self.data_idx);
        if self.len != 0 {
            for i in 0..self.len {
                if i as usize % BYTES_PER_LINE == BYTES_PER_LINE / 2 {
                    self.display_buf += " ";
                }
                self.display_buf += &format!("{:02X} ", self.buf[self.alt_buf][i]);
            }
            if self.len == BYTES_PER_LINE / 2 {
                self.display_buf += " "
            }
            for i in 0..(BYTES_PER_LINE - self.len) {
                if i as usize % BYTES_PER_LINE == BYTES_PER_LINE / 2 {
                    self.display_buf += " ";
                }
                self.display_buf += "   ";
            }
        } else {
            self.display_buf += &format!("{:49}", "");
        }
        if self.chars {
            self.display_buf += &format!("{LINE_SEP_VERT} ");
            if self.len != 0 {
                self.display_buf += CHAR_BORDER;
                for i in 0..self.len {
                    self.display_buf +=
                        &format!("{}", mask_chars(self.buf[self.alt_buf][i] as char));
                }
                self.display_buf += CHAR_BORDER;
            } else {
                self.display_buf += &format!("{:^8}", "");
            }
        }
        self.display();
    }

    /// skip duplicate lines and display skip info
    fn skip_lines(&mut self, data: &mut dyn DataSource) -> Result<()> {
        trace!(buf = format!("{:?}", self.buf), "found a duplicating line");
        let start_line = self.data_idx;
        while self.buf[0] == self.buf[1] && self.len == BYTES_PER_LINE {
            self.rd_data(data)?;
        }
        self.display_buf += &format!(
            "******** {LINE_SEP_VERT} {:<49}",
            format!(
                "(repeats {} lines)",
                self.data_idx - start_line / (BYTES_PER_LINE) + 1
            )
        );
        if self.chars {
            self.display_buf += &format!("{LINE_SEP_VERT}");
        }
        trace!(
            buf = format!("{:X?}", self.buf),
            "dumping buf after line skip"
        );
        self.display();
        Ok(())
    }

    /// dump a data sources contents according to the set configs
    pub fn dump(&mut self, data: &mut dyn DataSource) -> Result<()> {
        // skip a given number of bytes
        if self.skip > 0 {
            data.skip(self.skip)?;
            self.rd_counter += self.skip;
            debug!(
                data_idx = self.data_idx,
                "Skipped {}",
                humanbytes(self.skip)
            );
        }

        // print the head
        self.display_buf += &format!("DATA IDX {LINE_SEP_VERT} DATA AS HEX");
        if self.chars {
            self.display_buf += &format!("{:width$} {LINE_SEP_VERT} DATA AS CHAR", "", width = 37);
        }
        self.display();
        self.sep();

        // data dump loop
        self.rd_data(data)?;
        self.data_idx = 0;
        while self.len > 0 || self.first_iter {
            self.first_iter = false;

            self.dump_a_line();

            // loop breaker logic
            if self.stop || self.len < BYTES_PER_LINE {
                break;
            }
            self.rd_data(data)?;

            // after line logic
            if self.buf[0] == self.buf[1] && self.len == BYTES_PER_LINE && !self.show_identical {
                self.skip_lines(data)?;
            }
        }
        self.data_idx += BYTES_PER_LINE;

        self.sep();
        self.display_buf += &format!(
            "{:08X} {LINE_SEP_VERT} read total:\t\t    {:<8} {:<15}",
            self.rd_counter,
            humanbytes(self.rd_counter),
            format!("({} B)", self.rd_counter)
        );
        if self.chars {
            self.display_buf += &format!("{LINE_SEP_VERT}");
        }
        self.display();
        Ok(())
    }
    #[inline]
    fn adjust_counters(&mut self) {
        self.rd_counter += self.len;
        self.data_idx += self.len;
    }

    /// read data from the data source
    fn rd_data(&mut self, data: &mut dyn DataSource) -> Result<()> {
        match data.read(&mut self.buf[self.alt_buf]) {
            Ok(mut len) => {
                if self.limit != 0 && self.rd_counter + (BYTES_PER_LINE - 1) >= self.limit {
                    len = self.limit % BYTES_PER_LINE;
                    self.stop = true;
                }
                self.len = len;
                self.adjust_counters();
                Ok(())
            }
            Err(err) => {
                error!("error while reading data: {err}");
                Err(err.into())
            }
        }
    }
}

/// interpret characters for the --chars option
fn mask_chars(c: char) -> char {
    if c.is_ascii_graphic() {
        return c;
    } else if c == '\n' {
        return '↩';
    } else if c == ' ' {
        return '␣';
    } else if c == '\t' {
        return '⭾';
    } else {
        return '�';
    }
}
