//! SV39多级页表的地址约定

use crate::config::PAGE_SIZE;
use crate::config::PAGE_SIZE_BITS;

use core::fmt::LowerHex;
use core::iter::Step;
use core::ops::{Add, AddAssign, Shl};
use core::slice;

use super::PageTable;
use super::page_table;

/// satp的模式字段值为8时，会启用SV39分页模式
const SV39_MODE_MASK: usize = 0b1000 << 60;

/// 虚拟地址 (39位)
/// - [12:38] 虚拟页号
/// - [0:11]  对应物理页的页内偏移
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(usize);

/// 虚拟页号 (27位)
/// 1. [18:26] 一级索引，指示一级页表中二级页表的物理页号
/// 2. [9:17]  二级索引，指示二级页表中三级页表的物理页号
/// 3. [0:8]   三级索引，指示三级页表中目标位置的物理页号
///
/// 索引，即是页表内的偏移。
/// 页表位于物理页内，一个物理页大小为4K字节，
/// 而页表项占8字节，因此页表总共有512条表项。
/// log2(512) = 9，故索引的长度为9比特。
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtPageNum(usize);

/// 物理地址 (56位)
/// - [12:55] 物理页号
/// - [0:11]  页内偏移
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysPageNum(usize);

impl VirtAddr {
    pub const WIDTH: usize = 39;

    pub fn from_raw(va: usize) -> Self {
        Self(va)
    }

    pub fn page_number(&self) -> VirtPageNum {
        self.floor()
    }

    pub fn page_offset(&self) -> usize {
        // 左移多少位，就是在右侧留下多少个0。
        //
        // 1左移后减去1，这些0全部变成1，
        // 而先前最左侧的1消失
        self.0 & ((1 << PAGE_SIZE_BITS) - 1)
    }

    pub fn is_aligned(&self) -> bool {
        self.page_offset() == 0
    }

    /// 本虚拟地址的页号，区间的闭端
    pub fn floor(&self) -> VirtPageNum {
        // 十六进制下，一位有16种情况，
        // 故十六进制下3位涵盖二进制下
        // 3 * log2(16) = 3 * 4 = 12 位
        //
        // 地址除 0x1000 就是右移12位啦
        VirtPageNum(self.0 / PAGE_SIZE)
    }

    /// 本虚拟地址的上界页号，区间的开端
    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum(self.0.div_ceil(PAGE_SIZE))
    }
}

impl core::fmt::Debug for VirtPageNum {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

// 实现了就能用 Range<VirtPageNum> 迭代了
impl Step for VirtPageNum {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        usize::steps_between(&start.0, &end.0)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        usize::forward_checked(start.0, count).map(VirtPageNum)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        usize::backward_checked(start.0, count).map(VirtPageNum)
    }
}

impl VirtPageNum {
    const WIDTH: usize = VirtAddr::WIDTH - PAGE_SIZE_BITS;
    const INDEX_MASK: usize = 0b1_1111_1111;

    pub fn from_raw(vpn: usize) -> Self {
        Self(vpn)
    }

    /// 取出三个级别的索引
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut indexes = [0; 3];
        for i in indexes.iter_mut().rev() {
            *i = vpn & Self::INDEX_MASK;
            vpn >>= 9;
        }
        indexes
    }

    pub fn identity_map(self) -> PhysPageNum {
        PhysPageNum(self.0)
    }
}

impl PhysAddr {
    pub const WIDTH: usize = 56;

    pub fn page_number(&self) -> PhysPageNum {
        self.floor()
    }

    pub fn page_offset(&self) -> usize {
        // 1左移多少位，就是在右侧留下多少个0，
        // 再减去1，这些0全部变成1，而先前最左侧的1消失
        self.0 & ((1 << PAGE_SIZE_BITS) - 1)
    }

    pub fn is_aligned(&self) -> bool {
        self.page_offset() == 0
    }

    /// 本物理地址的页号，区间的闭端
    pub fn floor(&self) -> PhysPageNum {
        // 十六进制下，一位有16种情况，
        // 故十六进制下3位涵盖二进制下
        // 3 * log2(16) = 3 * 4 = 12 位
        //
        // 地址除 0x1000 就是右移12位啦
        PhysPageNum(self.0 / PAGE_SIZE)
    }

    /// 本物理地址的上界页号，区间的开端
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum(self.0.div_ceil(PAGE_SIZE))
    }

    pub fn as_ref<T>(self) -> &'static T {
        unsafe { (self.0 as *const T).as_ref().unwrap() }
    }

    pub fn as_mut<T>(self) -> &'static mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }
}

impl core::fmt::Debug for PhysPageNum {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

impl PhysPageNum {
    pub const WIDTH: usize = PhysAddr::WIDTH - PAGE_SIZE_BITS;

    pub fn from_raw(ppn: usize) -> Self {
        Self(ppn)
    }

    /// 得到用于传给 satp 的数据
    pub fn into_satp(self) -> usize {
        SV39_MODE_MASK | self.0
    }

    pub fn as_mut<T>(self) -> &'static mut T {
        PhysAddr::from(self).as_mut()
    }

    pub fn ptes_mut(self) -> &'static mut [page_table::Entry] {
        let pa = PhysAddr::from(self);
        unsafe { slice::from_raw_parts_mut(pa.0 as *mut page_table::Entry, PageTable::CAPACITY) }
    }

    /// 读出指定物理页的数据
    pub fn page_bytes(self) -> &'static [u8] {
        let pa = PhysAddr::from(self);
        unsafe { slice::from_raw_parts(pa.0 as *const u8, PAGE_SIZE) }
    }

    /// 读出指定物理页的数据以供修改
    pub fn page_bytes_mut(self) -> &'static mut [u8] {
        // 可见，[物理页号 0000_0000_0000] 即物理页的地址
        let pa = PhysAddr::from(self);
        unsafe { slice::from_raw_parts_mut(pa.0 as *mut u8, PAGE_SIZE) }
    }
}

// ========== VirtAddr * usize ==========

impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VirtAddr::WIDTH) - 1))
    }
}

impl From<VirtAddr> for usize {
    fn from(va: VirtAddr) -> Self {
        if va.0 >= (1 << (VirtAddr::WIDTH - 1)) {
            // 高256G，
            // 掩成前导1的形式，原因是`VirtAddr`只存了[0:38]，没存前导1
            va.0 | !((1 << VirtAddr::WIDTH) - 1)
        } else {
            // 低256G
            va.0
        }
    }
}

impl Add<usize> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

// ========== VirtPageNum * usize ==========

impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VirtPageNum::WIDTH) - 1))
    }
}

impl From<VirtPageNum> for usize {
    fn from(vpn: VirtPageNum) -> Self {
        vpn.0
    }
}

impl Add<usize> for VirtPageNum {
    type Output = VirtPageNum;

    fn add(self, rhs: usize) -> Self::Output {
        // 若为高256G，溢出会得到为0的虚拟页号
        Self::from(self.0 + rhs)
    }
}

impl AddAssign<usize> for VirtPageNum {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

// ========== PhysAddr * usize ==========

impl From<usize> for PhysAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PhysAddr::WIDTH) - 1))
    }
}

impl From<PhysAddr> for usize {
    fn from(pa: PhysAddr) -> Self {
        pa.0
    }
}

impl Add<usize> for PhysAddr {
    type Output = PhysAddr;

    fn add(self, rhs: usize) -> Self::Output {
        Self::from(self.0 + rhs)
    }
}

// ========== PhysPageNum * usize ==========

impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PhysPageNum::WIDTH) - 1))
    }
}

impl From<PhysPageNum> for usize {
    fn from(ppn: PhysPageNum) -> Self {
        ppn.0
    }
}

impl Shl<usize> for PhysPageNum {
    type Output = usize;

    fn shl(self, rhs: usize) -> Self::Output {
        self.0 << rhs
    }
}

impl Add<usize> for PhysPageNum {
    type Output = PhysPageNum;

    fn add(self, rhs: usize) -> Self::Output {
        Self::from(self.0 + rhs)
    }
}

impl AddAssign<usize> for PhysPageNum {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

// ========== Va <=> Vpn ==========

impl From<VirtAddr> for VirtPageNum {
    fn from(va: VirtAddr) -> Self {
        assert_eq!(va.page_offset(), 0);
        va.page_number()
    }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(vpn: VirtPageNum) -> Self {
        Self(vpn.0 << PAGE_SIZE_BITS)
    }
}

// ========== Pa <=> Ppn ==========

impl From<PhysAddr> for PhysPageNum {
    fn from(pa: PhysAddr) -> Self {
        assert_eq!(pa.page_offset(), 0);
        pa.page_number()
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(ppn: PhysPageNum) -> Self {
        Self(ppn.0 << PAGE_SIZE_BITS)
    }
}
