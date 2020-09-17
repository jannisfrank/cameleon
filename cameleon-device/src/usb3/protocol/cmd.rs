use std::{convert::TryInto, io::Write};

use byteorder::{WriteBytesExt, LE};

use crate::usb3::{Error, Result};

#[derive(Debug)]
pub struct CommandPacket<T> {
    ccd: CommandCcd,
    scd: T,
}

impl<T> CommandPacket<T>
where
    T: CommandScd,
{
    const PREFIX_MAGIC: u32 = 0x43563355;

    // Magic + CCD length.
    const ACK_HEADER_LENGTH: usize = 4 + 8;

    // Length of pending ack SCD. This SCD can be returned with any command.
    const MINIMUM_ACK_SCD_LENGTH: u16 = 4;

    pub fn serialize(&self, mut buf: impl Write) -> Result<()> {
        buf.write_u32::<LE>(Self::PREFIX_MAGIC)?;
        self.ccd.serialize(&mut buf)?;
        self.scd.serialize(&mut buf)?;

        Ok(())
    }

    pub fn ccd(&self) -> &CommandCcd {
        &self.ccd
    }

    pub fn scd(&self) -> &T {
        &self.scd
    }

    pub fn cmd_len(&self) -> usize {
        // Magic(4bytes) + ccd + scd
        4 + self.ccd.len() as usize + self.scd.scd_len() as usize
    }

    pub fn request_id(&self) -> u16 {
        self.ccd.request_id
    }

    /// Maximum length of corresponding ack packet.
    pub fn maximum_ack_len(&self) -> usize {
        let scd_len = self.scd.ack_scd_len();
        let maximum_scd_length = std::cmp::max(scd_len, Self::MINIMUM_ACK_SCD_LENGTH) as usize;

        Self::ACK_HEADER_LENGTH + maximum_scd_length
    }

    pub fn new(scd: T, request_id: u16) -> Self {
        let ccd = CommandCcd::from_scd(&scd, request_id);
        Self { ccd, scd }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadMem {
    pub(crate) address: u64,
    pub(crate) read_length: u16,
}

impl ReadMem {
    pub fn new(address: u64, read_length: u16) -> Self {
        Self {
            address,
            read_length,
        }
    }

    pub fn finalize(self, request_id: u16) -> CommandPacket<Self> {
        CommandPacket::new(self, request_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteMem<'a> {
    pub(crate) address: u64,
    pub(crate) data: &'a [u8],
    data_len: u16,
    len: u16,
}

impl<'a> WriteMem<'a> {
    pub fn new(address: u64, data: &'a [u8]) -> Result<Self> {
        let data_len = into_scd_len(data.len())?;
        let len = into_scd_len(data.len() + 8)?;

        Ok(Self {
            address,
            data,
            data_len,
            len,
        })
    }

    pub fn finalize(self, request_id: u16) -> CommandPacket<Self> {
        CommandPacket::new(self, request_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadMemStacked {
    pub(crate) entries: Vec<ReadMem>,
    len: u16,
    ack_scd_len: u16,
}

impl ReadMemStacked {
    pub fn new(entries: Vec<ReadMem>) -> Result<Self> {
        let len = Self::len(&entries)?;
        let ack_scd_len = Self::ack_scd_len(&entries)?;

        Ok(Self {
            entries,
            len,
            ack_scd_len,
        })
    }

    pub fn finalize(self, request_id: u16) -> CommandPacket<Self> {
        CommandPacket::new(self, request_id)
    }

    fn len(entries: &[ReadMem]) -> Result<u16> {
        let len = entries
            .iter()
            .fold(0, |acc, ent| acc + ent.scd_len() as usize);
        into_scd_len(len)
    }

    fn ack_scd_len(entries: &[ReadMem]) -> Result<u16> {
        let mut acc: u16 = 0;
        for ent in entries {
            acc = acc.checked_add(ent.read_length).ok_or_else(|| {
                Error::InvalidPacket("total read length must be less than u16::MAX".into())
            })?;
        }

        Ok(acc)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteMemStacked<'a> {
    pub(crate) entries: Vec<WriteMem<'a>>,
    len: u16,
    ack_scd_len: u16,
}

impl<'a> WriteMemStacked<'a> {
    pub fn new(entries: Vec<WriteMem<'a>>) -> Result<Self> {
        let len = Self::len(&entries)?;
        let ack_scd_len = entries.len() as u16 * 4;
        Ok(Self {
            entries,
            len,
            ack_scd_len,
        })
    }

    pub fn finalize(self, request_id: u16) -> CommandPacket<Self> {
        CommandPacket::new(self, request_id)
    }

    fn len(entries: &[WriteMem<'a>]) -> Result<u16> {
        let len = entries
            .iter()
            .fold(0, |acc, ent| acc + 12 + ent.data_len as usize);
        into_scd_len(len)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandCcd {
    flag: CommandFlag,
    scd_kind: ScdKind,
    scd_len: u16,
    request_id: u16,
}

impl CommandCcd {
    pub fn flag(&self) -> CommandFlag {
        self.flag
    }

    pub fn scd_kind(&self) -> ScdKind {
        self.scd_kind
    }

    pub fn scd_len(&self) -> u16 {
        self.scd_len
    }

    pub fn request_id(&self) -> u16 {
        self.request_id
    }

    pub(crate) fn new(flag: CommandFlag, scd_kind: ScdKind, scd_len: u16, request_id: u16) -> Self {
        Self {
            flag,
            scd_kind,
            scd_len,
            request_id,
        }
    }

    fn from_scd(scd: &impl CommandScd, request_id: u16) -> Self {
        Self::new(scd.flag(), scd.scd_kind(), scd.scd_len(), request_id)
    }

    fn serialize(&self, mut buf: impl Write) -> Result<()> {
        self.flag.serialize(&mut buf)?;
        self.scd_kind.serialize(&mut buf)?;
        buf.write_u16::<LE>(self.scd_len as u16)?;

        buf.write_u16::<LE>(self.request_id)?;
        Ok(())
    }

    const fn len(&self) -> u16 {
        // flags(2bytes) + command_id(2bytes) + scd_len(2bytes) + request_id(2bytes)
        8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandFlag {
    RequestAck,
    CommandResend,
}

impl CommandFlag {
    fn serialize(&self, mut buf: impl Write) -> Result<()> {
        let flag_id = match self {
            Self::RequestAck => 1 << 14,
            Self::CommandResend => 1 << 15,
        };
        Ok(buf.write_u16::<LE>(flag_id)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScdKind {
    ReadMem,
    WriteMem,
    ReadMemStacked,
    WriteMemStacked,
}

impl ScdKind {
    fn serialize(self, mut buf: impl Write) -> Result<()> {
        let kind_id = match self {
            Self::ReadMem => 0x0800,
            Self::WriteMem => 0x0802,
            Self::ReadMemStacked => 0x0806,
            Self::WriteMemStacked => 0x0808,
        };

        Ok(buf.write_u16::<LE>(kind_id)?)
    }
}

pub trait CommandScd: std::fmt::Debug {
    fn flag(&self) -> CommandFlag;

    fn scd_kind(&self) -> ScdKind;

    fn scd_len(&self) -> u16;

    fn serialize(&self, buf: impl Write) -> Result<()>;

    fn ack_scd_len(&self) -> u16;
}

impl CommandScd for ReadMem {
    fn flag(&self) -> CommandFlag {
        CommandFlag::RequestAck
    }

    fn scd_kind(&self) -> ScdKind {
        ScdKind::ReadMem
    }

    fn scd_len(&self) -> u16 {
        // Address(8 bytes) + reserved(2bytes) + length(2 bytes)
        12
    }

    fn serialize(&self, mut buf: impl Write) -> Result<()> {
        buf.write_u64::<LE>(self.address)?;
        buf.write_u16::<LE>(0)?; // 2bytes reserved.
        buf.write_u16::<LE>(self.read_length)?;
        Ok(())
    }

    fn ack_scd_len(&self) -> u16 {
        self.read_length
    }
}

impl<'a> CommandScd for WriteMem<'a> {
    fn flag(&self) -> CommandFlag {
        CommandFlag::RequestAck
    }

    fn scd_kind(&self) -> ScdKind {
        ScdKind::WriteMem
    }

    fn scd_len(&self) -> u16 {
        self.len
    }

    fn serialize(&self, mut buf: impl Write) -> Result<()> {
        buf.write_u64::<LE>(self.address)?;
        buf.write_all(self.data)?;
        Ok(())
    }

    fn ack_scd_len(&self) -> u16 {
        // Reserved(2bytes)+ length written(2bytes);
        4
    }
}

impl<'a> CommandScd for ReadMemStacked {
    fn flag(&self) -> CommandFlag {
        CommandFlag::RequestAck
    }

    fn scd_kind(&self) -> ScdKind {
        ScdKind::ReadMemStacked
    }

    fn scd_len(&self) -> u16 {
        self.len
    }

    fn serialize(&self, mut buf: impl Write) -> Result<()> {
        for ent in &self.entries {
            ent.serialize(&mut buf)?;
        }
        Ok(())
    }

    fn ack_scd_len(&self) -> u16 {
        self.ack_scd_len
    }
}

impl<'a> CommandScd for WriteMemStacked<'a> {
    fn flag(&self) -> CommandFlag {
        CommandFlag::RequestAck
    }

    fn scd_kind(&self) -> ScdKind {
        ScdKind::WriteMemStacked
    }

    fn scd_len(&self) -> u16 {
        // Each entry is composed of [address(8bytes), reserved(2bytes), data_len(2bytes), data(len bytes)]
        self.len
    }

    fn serialize(&self, mut buf: impl Write) -> Result<()> {
        for ent in &self.entries {
            buf.write_u64::<LE>(ent.address)?;
            buf.write_u16::<LE>(0)?; // 2bytes reserved.
            buf.write_u16::<LE>(ent.data_len)?;
            buf.write_all(ent.data)?;
        }
        Ok(())
    }

    fn ack_scd_len(&self) -> u16 {
        self.ack_scd_len
    }
}

fn into_scd_len(len: usize) -> Result<u16> {
    len.try_into()
        .map_err(|_| Error::InvalidPacket("scd length must be less than u16::MAX".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_LEN: u8 = 4 + 8; // Magic + CCD.

    fn serialize_header(command_id: &[u8; 2], scd_len: &[u8; 2], req_id: &[u8; 2]) -> Vec<u8> {
        let mut ccd = vec![];
        ccd.write_u32::<LE>(0x43563355).unwrap(); // Magic.
        ccd.extend(&[0x00, 0x40]); // Packet flag: Request Ack.
        ccd.extend(command_id);
        ccd.extend(scd_len);
        ccd.extend(req_id);
        ccd
    }

    #[test]
    fn test_read_mem_cmd() {
        let command = ReadMem::new(0x0004, 64).finalize(1);
        let scd_len = 12;

        assert_eq!(command.cmd_len(), (HEADER_LEN + scd_len).into());
        assert_eq!(command.request_id(), 1);

        let mut buf = vec![];
        command.serialize(&mut buf).unwrap();
        let mut expected = serialize_header(&[0x00, 0x08], &[scd_len, 0x00], &[0x01, 0x00]);
        expected.extend(vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Address.
        expected.extend(vec![0x00, 0x00]); // Reserved.
        expected.extend(vec![64, 0x00]); // Read length.

        assert_eq!(buf, expected);
    }

    #[test]
    fn test_write_mem_cmd() {
        let command = WriteMem::new(0x0004, &[0x01, 0x02, 0x03])
            .unwrap()
            .finalize(1);
        let scd_len = 11;

        assert_eq!(command.cmd_len(), (HEADER_LEN + scd_len).into());
        assert_eq!(command.request_id(), 1);

        let mut buf = vec![];
        command.serialize(&mut buf).unwrap();
        let mut expected = serialize_header(&[0x02, 0x08], &[scd_len, 0x00], &[0x01, 0x00]);
        expected.extend(vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Address.
        expected.extend(vec![0x01, 0x02, 0x03]); // Data.

        assert_eq!(buf, expected);
    }

    #[test]
    fn test_read_mem_stacked() {
        let mut read_mems = vec![];
        read_mems.push(ReadMem::new(0x0004, 4));
        read_mems.push(ReadMem::new(0x0008, 8));

        let command = ReadMemStacked::new(read_mems).unwrap().finalize(1);
        let scd_len = 12 * 2;

        assert_eq!(command.cmd_len(), (HEADER_LEN + scd_len).into());
        assert_eq!(command.request_id(), 1);

        let mut buf = vec![];
        command.serialize(&mut buf).unwrap();
        let mut expected = serialize_header(&[0x06, 0x08], &[12 * 2, 0x00], &[0x01, 0x00]);
        expected.extend(vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Address 0.
        expected.extend(vec![0x00, 0x00]); // Reserved 0.
        expected.extend(vec![4, 0x00]); // Read length 0.
        expected.extend(vec![0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Address 1.
        expected.extend(vec![0x00, 0x00]); // Reserved 1.
        expected.extend(vec![8, 0x00]); // Read length 1.

        assert_eq!(buf, expected);
    }

    #[test]
    fn test_write_mem_stacked() {
        let mut write_mems = vec![];
        write_mems.push(WriteMem::new(0x0004, &[0x01, 0x02, 0x03, 0x04]).unwrap());
        write_mems.push(WriteMem::new(0x0008, &[0x11, 0x12, 0x13, 0x14]).unwrap());

        let command = WriteMemStacked::new(write_mems).unwrap().finalize(1);
        let scd_len = (12 + 4) * 2;

        assert_eq!(command.cmd_len(), (HEADER_LEN + scd_len).into());
        assert_eq!(command.request_id(), 1);

        let mut buf = vec![];
        command.serialize(&mut buf).unwrap();
        let mut expected = serialize_header(&[0x08, 0x08], &[scd_len, 0x00], &[0x01, 0x00]);
        expected.extend(vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Address 0.
        expected.extend(vec![0x00, 0x00]); // Reserved 0.
        expected.extend(vec![4, 0x00]); // Length data block 0.
        expected.extend(vec![0x01, 0x02, 0x03, 0x04]); // Data block 0.
        expected.extend(vec![0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Address 1.
        expected.extend(vec![0x00, 0x00]); // Reserved 1.
        expected.extend(vec![4, 0x00]); // Length data block 1.
        expected.extend(vec![0x11, 0x12, 0x13, 0x14]); // Data block 1.

        assert_eq!(buf, expected);
    }
}
