#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PhysAddr(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);

impl VirtAddr {
    pub fn p4_index(self) -> usize {
        (self.0 >> 39) & 0o777
    }

    pub fn p3_index(self) -> usize {
        (self.0 >> 30) & 0o777
    }

    pub fn p2_index(self) -> usize {
        (self.0 >> 21) & 0o777
    }

    pub fn p1_index(self) -> usize {
        (self.0 >> 12) & 0o777
    }

    pub fn page_offset(self) -> usize {
        self.0 & 0xfff
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PteFlags(pub usize);

impl PteFlags {
    pub const PRESENT: Self = Self(1 << 0);
    pub const WRITABLE: Self = Self(1 << 1);
    pub const USER: Self = Self(1 << 2);
    pub const ACCESSED: Self = Self(1 << 5);
    pub const DIRTY: Self = Self(1 << 6);

    #[inline]
    pub const fn bits(self) -> usize {
        self.0
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableEntry(pub usize);

impl PageTableEntry {
    pub const FLAG_MASK: usize = 0xFFF;
    pub const ADDR_MASK: usize = !Self::FLAG_MASK;

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub fn new(frame: PhysAddr, flags: PteFlags) -> Self {
        assert_eq!(frame.0 & Self::FLAG_MASK, 0, "frame must be 4KiB aligned");
        Self(frame.0 | (flags.bits() & Self::FLAG_MASK))
    }

    #[inline]
    pub fn frame(self) -> PhysAddr {
        PhysAddr(self.0 & Self::ADDR_MASK)
    }

    #[inline]
    pub fn flags(self) -> PteFlags {
        PteFlags(self.0 & Self::FLAG_MASK)
    }

    #[inline]
    pub fn is_present(self) -> bool {
        self.flags().contains(PteFlags::PRESENT)
    }
}

#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub fn zeroed() -> Self {
        Self {
            entries: [PageTableEntry::empty(); 512],
        }
    }

    pub fn clear(&mut self) {
        self.entries.fill(PageTableEntry::empty());
    }

    pub fn set_entry(&mut self, index: usize, entry: PageTableEntry) {
        self.entries[index] = entry;
    }
}
