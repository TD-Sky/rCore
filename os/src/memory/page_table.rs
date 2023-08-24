use super::address::PhysAddr;
use super::address::PhysPageNum;
use super::address::VirtAddr;
use super::address::VirtPageNum;
use super::frame_allocator;
use super::frame_allocator::Frame;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use enumflags2::bitflags;
use enumflags2::BitFlags;

pub struct PageTable {
    /// 一级页表的物理地址，要交给satp
    root: PhysPageNum,
    frames: Vec<Frame>,
}

/// 页表项，根据物理地址查到页，
/// 该页装有页表，此乃表中之项
///
/// - [28:53] PPN[2]
/// - [19:27] PPN[1]
/// - [10:18] PPN[0]
/// - [8:9]   未知
/// - [0:7]   保护位
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Entry(usize);

/// 保护位
/// - V(Valid)：仅当位 V 为 1 时，页表项才是合法的；
/// - R(Read)/W(Write)/X(eXecute)：分别控制索引到这个页表项的对应虚拟页面是否允许读/写/执行；
/// - U(User)：控制索引到这个页表项的对应虚拟页面是否在 CPU 处于 U 特权级的情况下是否被允许访问；
/// - G：暂且不理会；
/// - A(Accessed)：处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被访问过；
/// - D(Dirty)：处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被修改过。
#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PTEFlag {
    V = 0b0000_0001,
    R = 0b0000_0010,
    W = 0b0000_0100,
    X = 0b0000_1000,
    U = 0b0001_0000,
    G = 0b0010_0000,
    A = 0b0100_0000,
    D = 0b1000_0000,
}

#[derive(Debug)]
pub struct MappedVpn(pub VirtPageNum);

#[derive(Debug)]
pub struct UnmappedVpn(pub VirtPageNum);

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTable {
    /// 可容纳的页表项数量
    pub const CAPACITY: usize = 512;

    pub fn new() -> Self {
        let frame = frame_allocator::alloc().unwrap();
        Self {
            root: frame.ppn,
            frames: vec![frame],
        }
    }

    /// 为`vpn`创建包含`ppn`的第三级表项
    pub fn map(
        &mut self,
        vpn: impl Into<VirtPageNum>,
        ppn: impl Into<PhysPageNum>,
        flags: BitFlags<PTEFlag>,
    ) -> Result<(), MappedVpn> {
        let vpn = vpn.into();

        let pte = self.get_or_insert(vpn);
        if pte.is_valid() {
            return Err(MappedVpn(vpn));
        }
        *pte = Entry::new(ppn.into(), flags | PTEFlag::V);

        Ok(())
    }

    /// 清空`vpn`映射的第三级表项
    pub fn unmap(&mut self, vpn: VirtPageNum) -> Result<(), UnmappedVpn> {
        let pte = self.get_mut(vpn).unwrap();
        if !pte.is_valid() {
            return Err(UnmappedVpn(vpn));
        }
        pte.clean();

        Ok(())
    }

    /// 凭借虚拟页号访问页表项
    pub fn translate(&self, vpn: VirtPageNum) -> Option<&Entry> {
        self.get_mut(vpn).map(|e| &*e)
    }

    pub fn translate_virt_addr(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.get_mut(va.page_number())
            .map(|pte| PhysAddr::from(pte.ppn()) + va.page_offset())
    }

    /// 将一级页表地址转化成 satp 使用的数据
    pub fn token(&self) -> usize {
        self.root.into_satp()
    }

    pub fn from_token(satp: usize) -> Self {
        Self {
            root: PhysPageNum::from(satp),
            frames: vec![],
        }
    }
}

impl PageTable {
    /// 根据虚拟页号查找三级页表项，并沿途创建尚未存在的页表项
    ///
    /// 注意：返回的页表项未做检查，可能无效
    fn get_or_insert(&mut self, vpn: VirtPageNum) -> &mut Entry {
        let indexes = vpn.indexes();
        let mut ppn = self.root;

        for (_i, &index) in indexes.iter().take(2).enumerate() {
            // debug!("level {} page table: {:#x}", i + 1, usize::from(ppn));
            // debug!("level {} index: {:#x}", i + 1, index);
            let pte = &mut ppn.ptes_mut()[index];
            if !pte.is_valid() {
                // 分配新的物理页，并让一/二级页表项指向此物理页。
                let frame = frame_allocator::alloc().unwrap();
                *pte = Entry::new(frame.ppn, PTEFlag::V);
                self.frames.push(frame);
            }

            ppn = pte.ppn();
        }

        let index = indexes[2];
        // debug!("level 3 page table: {:#x}", usize::from(ppn));
        // debug!("level 3 index: {:#x}", index);
        &mut ppn.ptes_mut()[index]
    }

    /// 根据虚拟页号查找三级表项，沿途若有无效表项，则返回 None。
    /// self是不可变引用，但返回的是可变借用，须防备读写出问题。
    ///
    /// 注意：返回的页表项未做检查，可能无效
    fn get_mut(&self, vpn: VirtPageNum) -> Option<&mut Entry> {
        let indexes = vpn.indexes();
        let mut ppn = self.root;

        for &i in indexes.iter().take(2) {
            let pte = &mut ppn.ptes_mut()[i];
            if !pte.is_valid() {
                return None;
            }

            ppn = pte.ppn();
        }

        Some(&mut ppn.ptes_mut()[indexes[2]])
    }
}

impl Entry {
    pub fn new(ppn: PhysPageNum, flags: impl Into<BitFlags<PTEFlag>>) -> Self {
        Self(ppn << 10 | flags.into().bits() as usize)
    }

    pub fn clean(&mut self) {
        *self = Self(0);
    }

    pub fn ppn(&self) -> PhysPageNum {
        PhysPageNum::from_raw(self.0 >> 10 & ((1 << PhysPageNum::WIDTH) - 1))
    }

    pub fn flags(&self) -> BitFlags<PTEFlag> {
        // 缩减usize为u8，得到低8位的二进制标志位
        BitFlags::from_bits(self.0 as u8).unwrap()
    }

    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlag::V)
    }
}

/// 翻译虚拟内存的指针，集合来自不同物理页的字节流以组成连续的字节流
pub fn read_bytes(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut bytes = vec![];

    while start < end {
        let start_va = VirtAddr::from(start);
        let vpn = start_va.page_number();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        let end_va = VirtAddr::from(end).min(VirtAddr::from(vpn + 1));

        if end_va.page_offset() == 0 {
            // 跨页了，先读完当前页所有
            bytes.push(&mut ppn.page_bytes_mut()[start_va.page_offset()..]);
        } else {
            // 同一页内
            bytes.push(&mut ppn.page_bytes_mut()[start_va.page_offset()..end_va.page_offset()]);
        }

        start = end_va.into();
    }

    bytes
}

pub fn read_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;

    loop {
        let ch: u8 = *(page_table.translate_virt_addr(va.into()).unwrap().as_mut());

        if ch != b'\0' {
            string.push(ch as char);
            va += 1;
        } else {
            break;
        }
    }

    string
}

pub fn read_mut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;

    page_table.translate_virt_addr(va.into()).unwrap().as_mut()
}
