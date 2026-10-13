use super::Result;
use ottd_sim::content::grf::{
    GrfStaticInfo, Palette, ParameterType, ScanFailure, ScanOptions, ScanStatus, TextList,
    scan_file_with_options,
};
use serde_json::{Value, json};

fn texts(list: &TextList<'_>) -> Value {
    json!({"entries":list.entries().iter().map(|text|json!({"language":text.language,"translated":text.translated})).collect::<Vec<_>>(),
        "selections":([0,1,2,127,255].map(|language|json!({"language":language,"text":list.select(language).map(|text|&text.translated)})))})
}
fn metadata(info: &GrfStaticInfo<'_>) -> Value {
    let compatibility: Vec<_> = [0, 1, 2, 3, 4, 6, 7, 8, u32::MAX - 1, u32::MAX]
        .into_iter()
        .map(|version| json!({"version":version,"compatible":info.is_compatible(version)}))
        .collect();
    let parameters:Vec<_>=info.parameters.iter().map(|entry|entry.as_ref().map(|p|json!({"slot":p.slot,"first_bit":p.first_bit,"num_bits":p.num_bits,"kind":match p.kind {ParameterType::UintEnum=>0,ParameterType::Bool=>1},"min":p.min,"max":p.max,"default":p.default,"complete_labels":p.complete_labels,"name":texts(&p.name),"description":texts(&p.description),"value_names":p.value_names.iter().map(|(value,list)|json!({"value":value,"texts":texts(list)})).collect::<Vec<_>>()}))).collect();
    json!({"name":texts(&info.name),"description":texts(&info.description),"url":texts(&info.url),"version":info.version,"min_loadable_version":info.min_loadable_version,"num_valid_params":info.num_valid_params,"palette_bits":info.palette_bits,"has_param_defaults":info.has_param_defaults,"parameters":parameters,"compatibility":compatibility})
}
fn values(info: &GrfStaticInfo<'_>, parameters: &[u32]) -> Value {
    json!({"parameters":parameters,"reads":info.parameters.iter().map(|p|p.as_ref().map(|p|p.read(parameters))).collect::<Vec<_>>()})
}
/// Project Rust metadata and explicit parameter operations.
/// # Errors
/// Returns malformed input, framing overflow or projection errors.
pub fn scan(bytes: &[u8], name: &str, palette: u8) -> Result<(Value, Value)> {
    let outcome = scan_file_with_options(
        bytes,
        ScanOptions {
            default_palette: if palette == 1 {
                Palette::Windows
            } else {
                Palette::Dos
            },
            ..ScanOptions::default()
        },
    )?;
    let info = &outcome.static_info;
    let supplied = vec![0x1234_5678, 0xffff_ffff, 0x0102_0304];
    let mut parameters = supplied.clone();
    let mut writes = Vec::new();
    let mut operations = Vec::new();
    for (index, entry) in info.parameters.iter().enumerate() {
        if let Some(parameter) = entry {
            for value in [0, 1, 3, 7, u32::MAX] {
                operations.push(json!({"index":index,"value":value}));
                parameter.write(&mut parameters, value);
                writes.push(values(info, &parameters));
            }
        }
    }
    let failure=outcome.failure.map(|(reason,line)|json!({"reason":match reason {ScanFailure::ReadBounds=>"ReadBounds",ScanFailure::UnexpectedSprite=>"UnexpectedSprite"},"line":line}));
    let result = json!({"name":name,"accepted":outcome.accepted,"status":match outcome.status {ScanStatus::Unknown=>0,ScanStatus::Disabled=>1},"grfid":outcome.metadata.as_ref().map_or(0,|v|v.grfid),"invalid_version":outcome.invalid_version,"system":outcome.system,"failure":failure,"metadata":metadata(info),"initial":values(info,&[]),"defaults":values(info,&info.default_parameters()),"supplied":values(info,&supplied),"writes":writes});
    Ok((
        result,
        json!({"name":name,"palette":palette,"parameters":supplied,"writes":operations}),
    ))
}
