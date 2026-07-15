use trpg_shared_kernel::EventEnvelope;

fn main() {
    let _: EventEnvelope<String> = EventEnvelope {
        integrity_hash: [0_u8; 32],
        ..panic!("the private field must be rejected at compile time")
    };
}
