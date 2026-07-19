// Impedisce l'apertura di una console su Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    locus_forge_lib::run()
}
