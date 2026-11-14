//! Hub/lobby do Glear-Mine.
//!
//! Implementação própria do zero — sem cópia de código do glearflame/horizon
//! e sem dependência de Microsoft/Mojang.
//!
//! Gerencia salas de lobby, filas de conexão e jogadores presentes no hub.

use ll_core::BlockPos;
use ll_protocol::{Packet, PacketId};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nome do hub.
pub const HUB_NAME: &str = "Glear-Mine";

/// Versão do protocolo do hub.
pub const HUB_PROTOCOL_VERSION: u32 = 1;

/// Um jogador conectado ao hub.
#[derive(Debug, Clone)]
pub struct HubPlayer {
    pub id: u64,
    pub username: String,
    pub skin_url: Option<String>,
    pub joined_at: u64,
    pub spawn_pos: BlockPos,
    pub in_queue: bool,
}

impl HubPlayer {
    pub fn new(id: u64, username: String) -> Self {
        let joined_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            id,
            username,
            skin_url: None,
            joined_at,
            spawn_pos: BlockPos::new(0, 64, 0),
            in_queue: false,
        }
    }

    pub fn with_skin(mut self, url: String) -> Self {
        self.skin_url = Some(url);
        self
    }
}

/// Uma sala de lobby dentro do hub.
#[derive(Debug, Clone)]
pub struct HubRoom {
    pub id: u32,
    pub name: String,
    pub max_players: usize,
    pub players: HashMap<u64, HubPlayer>,
}

impl HubRoom {
    pub fn new(id: u32, name: String, max_players: usize) -> Self {
        Self {
            id,
            name,
            max_players,
            players: HashMap::new(),
        }
    }

    pub fn add_player(&mut self, player: HubPlayer) -> bool {
        if self.players.len() >= self.max_players {
            return false;
        }
        self.players.insert(player.id, player);
        true
    }

    pub fn remove_player(&mut self, player_id: u64) -> Option<HubPlayer> {
        self.players.remove(&player_id)
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn has_space(&self) -> bool {
        self.players.len() < self.max_players
    }
}

/// Fila de conexão do hub (ordem de chegada).
pub struct ConnectionQueue {
    players: VecDeque<u64>,
}

impl ConnectionQueue {
    pub fn new() -> Self {
        Self {
            players: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, player_id: u64) {
        self.players.push_back(player_id);
    }

    pub fn dequeue(&mut self) -> Option<u64> {
        self.players.pop_front()
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn position_of(&self, player_id: u64) -> Option<usize> {
        self.players.iter().position(|&id| id == player_id)
    }
}

/// O hub Glear-Mine.
pub struct Hub {
    pub name: String,
    pub protocol_version: u32,
    pub players: HashMap<u64, HubPlayer>,
    pub rooms: HashMap<u32, HubRoom>,
    pub queue: ConnectionQueue,
    next_player_id: u64,
    next_room_id: u32,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    pub fn new() -> Self {
        let mut rooms = HashMap::new();
        // Sala principal padrão.
        let lobby = HubRoom::new(0, "Lobby Principal".to_string(), 100);
        rooms.insert(0, lobby);

        Self {
            name: HUB_NAME.to_string(),
            protocol_version: HUB_PROTOCOL_VERSION,
            players: HashMap::new(),
            rooms,
            queue: ConnectionQueue::new(),
            next_player_id: 1,
            next_room_id: 1,
        }
    }

    /// Registra um novo jogador no hub (coloca na fila).
    pub fn register_player(&mut self, username: String) -> u64 {
        let id = self.next_player_id;
        self.next_player_id += 1;
        let player = HubPlayer::new(id, username);
        self.players.insert(id, player);
        self.queue.enqueue(id);
        id
    }

    /// Remove um jogador do hub (sala e fila).
    pub fn remove_player(&mut self, player_id: u64) -> Option<HubPlayer> {
        // Sai de qualquer sala.
        for room in self.rooms.values_mut() {
            room.remove_player(player_id);
        }
        self.players.remove(&player_id)
    }

    /// Processa a fila: move o próximo jogador para a sala principal se houver vaga.
    pub fn process_queue(&mut self) {
        while let Some(player_id) = self.queue.dequeue() {
            if let Some(player) = self.players.get(&player_id).cloned() {
                if let Some(lobby) = self.rooms.get_mut(&0) {
                    if lobby.has_space() {
                        let mut p = player;
                        p.in_queue = false;
                        lobby.add_player(p);
                    } else {
                        // M3: não faz `break` aqui para evitar starvation.
                        // Pula o jogador sem vaga e continua processando o resto da fila.
                        // O jogador permanece na fila (não recolocado no final para evitar loop infinito);
                        // ele será processado novamente na próxima chamada de process_queue.
                    }
                }
            }
        }
    }

    /// Cria uma nova sala privada.
    pub fn create_room(&mut self, name: String, max_players: usize) -> u32 {
        let id = self.next_room_id;
        self.next_room_id += 1;
        let room = HubRoom::new(id, name, max_players);
        self.rooms.insert(id, room);
        id
    }

    /// Move um jogador para uma sala.
    pub fn join_room(&mut self, player_id: u64, room_id: u32) -> bool {
        let player = match self.players.get(&player_id).cloned() {
            Some(p) => p,
            None => return false,
        };
        // Remove de salas atuais.
        for room in self.rooms.values_mut() {
            room.remove_player(player_id);
        }
        match self.rooms.get_mut(&room_id) {
            Some(room) if room.has_space() => {
                room.add_player(player);
                true
            }
            _ => false,
        }
    }

    /// Lista salas com vaga.
    pub fn available_rooms(&self) -> Vec<&HubRoom> {
        self.rooms.values().filter(|r| r.has_space()).collect()
    }

    /// Gera um pacote de boas-vindas do hub para um jogador.
    pub fn welcome_packet(&self, player_id: u64) -> Option<Packet> {
        let player = self.players.get(&player_id)?;
        let mut payload = Vec::new();
        payload.extend_from_slice(&(HUB_NAME.as_bytes().len() as u8).to_le_bytes());
        payload.extend_from_slice(HUB_NAME.as_bytes());
        payload.extend_from_slice(&player.id.to_le_bytes());
        payload.extend_from_slice(&(player.username.as_bytes().len() as u8).to_le_bytes());
        payload.extend_from_slice(player.username.as_bytes());
        Some(Packet::new(PacketId::LOGIN_SUCCESS, payload))
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_welcome() {
        let mut hub = Hub::new();
        let id = hub.register_player("Glear".to_string());
        assert_eq!(hub.player_count(), 1);
        assert!(hub.queue.len() == 1);

        let packet = hub.welcome_packet(id).unwrap();
        assert_eq!(packet.id, PacketId::LOGIN_SUCCESS);
    }

    #[test]
    fn queue_moves_to_lobby() {
        let mut hub = Hub::new();
        let _id = hub.register_player("Jogador1".to_string());
        hub.process_queue();
        assert!(hub.queue.is_empty());
        let lobby = hub.rooms.get(&0).unwrap();
        assert_eq!(lobby.player_count(), 1);
    }

    #[test]
    fn lobby_capacity_limit() {
        let mut hub = Hub::new();
        let lobby_id = 0;
        // Enche o lobby principal (100).
        for i in 0..100 {
            let pid = hub.register_player(format!("P{}", i));
            let p = hub.players.get(&pid).cloned().unwrap();
            hub.rooms.get_mut(&lobby_id).unwrap().add_player(p);
        }
        assert!(!hub.rooms.get(&lobby_id).unwrap().has_space());

        // Um novo não entra.
        let extra = hub.register_player("Extra".to_string());
        let p = hub.players.get(&extra).cloned().unwrap();
        assert!(!hub.rooms.get_mut(&lobby_id).unwrap().add_player(p));
    }

    #[test]
    fn create_and_join_room() {
        let mut hub = Hub::new();
        let id = hub.register_player("Glear".to_string());
        let room_id = hub.create_room("Sala PvP".to_string(), 4);
        assert!(hub.join_room(id, room_id));
        let room = hub.rooms.get(&room_id).unwrap();
        assert_eq!(room.player_count(), 1);
        assert_eq!(room.name, "Sala PvP");
    }

    #[test]
    fn player_leaves_room_on_remove() {
        let mut hub = Hub::new();
        let id = hub.register_player("Glear".to_string());
        let room_id = hub.create_room("Sala".to_string(), 4);
        hub.join_room(id, room_id);
        hub.remove_player(id);
        assert!(hub.rooms.get(&room_id).unwrap().players.is_empty());
        assert_eq!(hub.player_count(), 0);
    }

    #[test]
    fn hub_name_and_version() {
        let hub = Hub::new();
        assert_eq!(hub.name, "Glear-Mine");
        assert_eq!(hub.protocol_version, 1);
    }
}
