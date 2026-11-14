//! Executável do hub Glear-Mine.

use ll_hub::Hub;

fn main() {
    println!("=== {} ===", ll_hub::HUB_NAME);
    println!("Hub/lobby próprio do Glear-Mine");
    println!("Protocolo v{}", ll_hub::HUB_PROTOCOL_VERSION);
    println!("Implementação independente do zero (sem terceiros).\n");

    let mut hub = Hub::new();

    // Registra jogadores de exemplo.
    let glear = hub.register_player("Glear".to_string());
    let _amiga = hub.register_player("Amiga123".to_string());
    let _amigo = hub.register_player("Amigo456".to_string());

    println!("Jogadores na fila: {}", hub.queue.len());

    // Processa a fila -> entra no lobby principal.
    hub.process_queue();
    println!("Após processar fila:");
    println!("  Fila: {}", hub.queue.len());
    println!("  Lobby principal: {} jogador(es)", hub.rooms.get(&0).unwrap().player_count());

    // Cria uma sala privada e move o Glear.
    let sala_id = hub.create_room("Sala PvP".to_string(), 4);
    hub.join_room(glear, sala_id);
    println!(
        "  Sala 'Sala PvP': {} jogador(es)",
        hub.rooms.get(&sala_id).unwrap().player_count()
    );

    // Pacote de boas-vindas.
    if let Some(pkt) = hub.welcome_packet(glear) {
        println!(
            "  Pacote de boas-vindas para Glear: {} bytes",
            pkt.payload.len()
        );
    }

    println!("\nTotal de jogadores: {}", hub.player_count());
    println!("Total de salas: {}", hub.room_count());
    println!("\nHub {} online.", ll_hub::HUB_NAME);
}
