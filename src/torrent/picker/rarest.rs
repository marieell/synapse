// Implementation based off of http://blog.libtorrent.org/2011/11/writing-a-fast-piece-picker/

use std::collections::HashSet;
use torrent::{Info, Peer, Bitfield, picker};
use std::ops::IndexMut;
use control::cio;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Picker {
    /// Current order of pieces
    pieces: Vec<u32>,
    /// Indices into pieces which indicate priority bounds
    priorities: Vec<usize>,
    /// Index mapping a piece to a position in the pieces field
    piece_idx: Vec<PieceInfo>,
    /// Set of peers which are seeders, and are not included in availability calcs
    seeders: HashSet<usize>,
}

enum PieceStatus {
    Incomplete,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PieceInfo {
    idx: usize,
    availability: usize,
    status: PieceStatus,
}

const PIECE_COMPLETE_INC: usize = 100;

impl Picker {
    pub fn new(pieces: &Bitfield) -> Picker {
        let mut piece_idx = Vec::new();
        for i in 0..pieces.len() {
            piece_idx.push(PieceInfo {
                idx: i as usize,
                availability: 0,
                status: PieceStatus::Incomplete,
            });
        }
        let mut p = Picker {
            seeders: HashSet::new(),
            pieces: (0..info.pieces()).collect(),
            piece_idx,
            priorities: vec![info.pieces() as usize],
        };
        // Start every piece at an availability of 1.
        // This way when we decrement availability for an initial
        // pick we never underflow, and can keep track of which pieces
        // are unpicked(odd) and picked(even).
        // We additionally mark pieces as properly completed
        for i in 0..pieces.len() {
            p.piece_available(i as u32);
            if pieces.has_bit(i) {
                p.completed(i);
            }
        }
        p
    }

    pub fn add_peer<T: cio::CIO>(&mut self, peer: &Peer<T>) {
        // Ignore seeders for availability calc
        if peer.pieces().complete() {
            self.seeders.insert(peer.id());
            return;
        }
        for idx in peer.pieces().iter() {
            self.piece_available(idx as u32);
        }
    }

    pub fn remove_peer<T: cio::CIO>(&mut self, peer: &Peer<T>) {
        if self.seeders.contains(&peer.id()) {
            self.seeders.remove(&peer.id());
            return;
        }
        for idx in peer.pieces().iter() {
            self.piece_unavailable(idx as u32);
        }
    }

    pub fn piece_available(&mut self, piece: u32) {
        let (idx, avail) = {
            let piece = self.piece_idx.index_mut(piece as usize);
            self.priorities[piece.availability] -= 1;
            piece.availability += 1;
            if self.priorities.len() == piece.availability {
                self.priorities.push(self.pieces.len());
            }
            (piece.idx, piece.availability - 1)
        };

        let swap_idx = self.priorities[avail];
        self.swap_piece(idx, swap_idx);
    }

    pub fn piece_unavailable(&mut self, piece: u32) {
        let (idx, avail) = {
            let piece = self.piece_idx.index_mut(piece as usize);
            piece.availability -= 1;
            self.priorities[piece.availability] += 1;
            (piece.idx, piece.availability)
        };

        let swap_idx = self.priorities[avail - 1];
        self.swap_piece(idx, swap_idx);
    }

    pub fn pick<T: cio::CIO>(&mut self, peer: &Peer<T>) -> Option<u32> {
        // Find the first matching piece which is not complete,
        // and that the peer also has
        self.pieces.iter()
            .cloned()
            .filter(|p| self.piece_idx[p].status == PieceStatus::Incomplete)
            .find(|p| peer.pieces().has_bit(p as u64))
            .map(|p| {
                if (self.piece_idx[p].availability % 2) == 0 {
                    self.piece_unavailable(p);
                }
                p
            })
                /*
                or bidx in 0..self.c.scale {
                    let block = *pidx as u64 * self.c.scale + bidx;
                    if !self.c.blocks.has_bit(block) {
                        self.c.blocks.set_bit(block);
                        let mut hs = HashSet::with_capacity(1);
                        hs.insert(peer.id());
                        self.c.waiting_peers.insert(block, hs);
                        self.c.waiting.insert(block);
                        if self.c.endgame_cnt == 1 {
                            // println!("Entering endgame!");
                        }
                        self.c.endgame_cnt = self.c.endgame_cnt.saturating_sub(1);
                        return Some(picker::Block {
                            index: *pidx as u32,
                            offset: bidx as u32 * 16384,
                        });
                    }
                }
                */
    }

    pub fn incomplete(&mut self, piece: u32) {
        self.piece_idx[piece as usize].status = PieceStatus::Incomplete;
        for _ in 0..PIECE_COMPLETE_INC {
            self.piece_unavailable(piece);
        }
    }

    pub fn completed(&mut self, piece: u32) {
        self.piece_idx[piece as usize].status = PieceStatus::Complete;
        // As hacky as this is, it's a good way to ensure that
        // we never waste time picking already selected pieces
        for _ in 0..PIECE_COMPLETE_INC {
            self.piece_available(piece);
        }
        //let idx: u64 = oidx as u64 * self.c.scale;
        //let offset: u64 = offset as u64 / 16384;
        //let block = idx + offset;
        //self.c.waiting.remove(&block);
        //let peers = self.c.waiting_peers.remove(&block).unwrap_or_else(|| {
        //    HashSet::with_capacity(0)
        //});
        //for i in 0..self.c.scale {
        //    if (idx + i < self.c.blocks.len() && !self.c.blocks.has_bit(idx + i)) ||
        //        self.c.waiting.contains(&(idx + i))
        //    {
        //        return (false, peers);
        //    }
        //}

        // TODO: Make this less hacky somehow
        // let pri_idx = self.piece_idx[oidx as usize].availability;
        // let pinfo_idx = self.piece_idx[oidx as usize].idx;
        // for pri in self.priorities.iter_mut() {
        //     if *pri > pri_idx as usize {
        //         *pri -= 1;
        //     }
        // }
        // for pinfo in self.piece_idx.iter_mut() {
        //     if pinfo.idx > pinfo_idx {
        //         pinfo.idx -= 1;
        //     }
        // }
        // self.pieces.remove(pinfo_idx);
        (true, peers)
    }

    fn swap_piece(&mut self, a: u32, b: u32) {
        self.piece_idx[self.pieces[a]].idx = b;
        self.piece_idx[self.pieces[b]].idx = a;
        self.pieces.swap(a, b);
    }

}

#[cfg(test)]
mod tests {
    use super::super::Block;
    use super::Picker;
    use torrent::{Info, Peer, Bitfield};

    #[test]
    fn test_available() {
        let info = Info::with_pieces(3);

        let b = Bitfield::new(3);
        let mut picker = Picker::new(&info, &b);
        let mut peers = vec![
            Peer::test_from_pieces(0, b.clone()),
            Peer::test_from_pieces(0, b.clone()),
            Peer::test_from_pieces(0, b.clone()),
        ];
        assert_eq!(picker.pick(&peers[0]), None);

        peers[0].pieces_mut().set_bit(0);
        peers[1].pieces_mut().set_bit(0);
        peers[1].pieces_mut().set_bit(2);
        peers[2].pieces_mut().set_bit(1);

        for peer in peers.iter() {
            picker.add_peer(peer);
        }
        assert_eq!(picker.pick(&peers[1]), Some(Block::new(2, 0)));
        assert_eq!(picker.pick(&peers[1]), Some(Block::new(0, 0)));
        assert_eq!(picker.pick(&peers[1]), None);
        assert_eq!(picker.pick(&peers[0]), None);
        assert_eq!(picker.pick(&peers[2]), Some(Block::new(1, 0)));
    }

    #[test]
    fn test_unavailable() {
        let b = Bitfield::new(3);
        let info = Info::with_pieces(3);

        let mut picker = Picker::new(&info, &b);
        let mut peers = vec![
            Peer::test_from_pieces(0, b.clone()),
            Peer::test_from_pieces(0, b.clone()),
            Peer::test_from_pieces(0, b.clone()),
        ];
        assert_eq!(picker.pick(&peers[0]), None);

        peers[0].pieces_mut().set_bit(0);
        peers[0].pieces_mut().set_bit(1);
        peers[1].pieces_mut().set_bit(1);
        peers[1].pieces_mut().set_bit(2);
        peers[2].pieces_mut().set_bit(0);
        peers[2].pieces_mut().set_bit(1);

        for peer in peers.iter() {
            picker.add_peer(peer);
        }
        picker.remove_peer(&peers[0]);

        assert_eq!(picker.pick(&peers[1]), Some(Block::new(2, 0)));
        assert_eq!(picker.pick(&peers[2]), Some(Block::new(0, 0)));
        assert_eq!(picker.pick(&peers[2]), Some(Block::new(1, 0)));

        picker.completed(0, 0);
        picker.completed(1, 0);
        picker.completed(2, 0);
        assert_eq!(picker.pick(&peers[1]), None);
        // picker.invalidate_piece(1);
        // assert_eq!(picker.pick(&peers[1]), Some(Block::new(1, 0)));
    }
}
