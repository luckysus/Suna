//! Isolated fixture; production uses the existing authorized file writer.
use std::{
    fs::OpenOptions,
    io::{Read, Write},
};
use suna_research_package_isolated::{archive, digest, projection, temporary};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("synthetic manifest and exclusive output required".into());
    }
    if arguments[0] == "--inspect" {
        let mut bytes = Vec::new();
        std::fs::File::open(&arguments[1])?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let checked = archive::inspect(&bytes)?;
        let simulation = temporary::simulate_import(&checked)?;
        println!(
            "{}",
            serde_json::json!({"receiptSha256":checked.receipt_sha256,"recordCount":checked.objects.len(),"edgeCount":checked.edges.len(),"status":simulation["status"],"idempotenceVerified":simulation["idempotenceVerified"],"writesToLiveWorkspace":false,"assetsRestored":false})
        );
        return Ok(());
    }
    let mut source = Vec::new();
    std::fs::File::open(&arguments[0])?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut source)?;
    if source.len() > 8 * 1024 * 1024 {
        return Err("synthetic source budget exceeded".into());
    }
    let manifest = serde_json::from_slice(&source)?;
    let bytes = archive::export(projection::from_manifest(&manifest)?)?;
    let checked = archive::inspect(&bytes)?;
    let simulation = temporary::simulate_import(&checked)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[1])?;
    output.write_all(&bytes)?;
    println!(
        "{}",
        serde_json::json!({"packageSha256":digest(&bytes),"receiptSha256":checked.receipt_sha256,
        "recordCount":checked.objects.len(),"edgeCount":checked.edges.len(),"status":simulation["status"],
        "idempotenceVerified":simulation["idempotenceVerified"],"writesToLiveWorkspace":false,"assetsRestored":false})
    );
    Ok(())
}
