//! DWARF v4 emission for Unix object targets.
//!
//! This module deliberately emits DWARF through `gimli` rather than writing
//! section bytes by hand.  Addresses are supplied by the object symbol table
//! reconciliation step, so the generated DIE ranges are tied to real
//! functions.  The CLI remains responsible for inserting the resulting
//! sections into the object container and for validating relocations on the
//! target platform.

use gimli::write::{
    Address, AttributeValue, Dwarf, DwarfUnit, EndianVec, Expression, LineProgram, LineString,
    Range, RangeList, Sections,
};
use gimli::{constants, Encoding, Format, LineEncoding, LittleEndian};

use crate::debug::CodeViewFunction;

pub type DwarfSection = (String, Vec<u8>);

pub fn sections_for_functions(
    source_file: &str,
    source: &str,
    functions: &[CodeViewFunction],
) -> Result<Vec<DwarfSection>, String> {
    let encoding = Encoding { format: Format::Dwarf32, version: 4, address_size: 8 };
    let file_name = std::path::Path::new(source_file)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(source_file);
    let mut line_program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(Vec::new()),
        None,
        LineString::String(file_name.as_bytes().to_vec()),
        None,
    );
    let directory = line_program.default_directory();
    let file = line_program.add_file(LineString::String(file_name.as_bytes().to_vec()), directory, None);
    for function in functions {
        line_program.begin_sequence(Some(Address::Constant(function.offset as u64)));
        let line_count = source.lines().count().max(1) as u32;
        for line in 0..line_count {
            line_program.set_address(Address::Constant(
                function.offset as u64
                    + (function.size.saturating_sub(1) as u64 * line as u64)
                        / line_count.saturating_sub(1).max(1) as u64,
            ));
            line_program.row().file = file;
            line_program.row().line = line as u64 + 1;
            line_program.generate_row();
        }
        line_program.end_sequence(function.offset as u64 + function.size.max(1) as u64);
    }

    let mut dwarf = DwarfUnit::new(encoding);
    dwarf.unit.line_program = line_program;
    let root = dwarf.unit.root();
    dwarf.unit.get_mut(root).set(constants::DW_AT_name, AttributeValue::String(file_name.as_bytes().to_vec()));
    dwarf.unit.get_mut(root).set(constants::DW_AT_comp_dir, AttributeValue::String(Vec::new()));
    dwarf.unit.get_mut(root).set(constants::DW_AT_producer, AttributeValue::String(b"SpectraLang".to_vec()));
    dwarf.unit.get_mut(root).set(constants::DW_AT_language, AttributeValue::Language(constants::DW_LANG_Rust));
    dwarf.unit.get_mut(root).set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

    for function in functions {
        let subprogram = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        dwarf.unit.get_mut(subprogram).set(
            constants::DW_AT_name,
            AttributeValue::String(function.name.as_bytes().to_vec()),
        );
        dwarf.unit.get_mut(subprogram).set(
            constants::DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(function.offset as u64)),
        );
        dwarf.unit.get_mut(subprogram).set(
            constants::DW_AT_high_pc,
            AttributeValue::Address(Address::Constant((function.offset + function.size.max(1)) as u64)),
        );
        let ranges = dwarf.unit.ranges.add(RangeList(vec![Range::StartLength {
            begin: Address::Constant(function.offset as u64),
            length: function.size.max(1) as u64,
        }]));
        dwarf.unit.get_mut(subprogram).set(constants::DW_AT_ranges, AttributeValue::RangeListRef(ranges));
        dwarf.unit.get_mut(subprogram).set(constants::DW_AT_decl_file, AttributeValue::FileIndex(Some(file)));
        dwarf.unit.get_mut(subprogram).set(constants::DW_AT_decl_line, AttributeValue::Udata(1));

        // A location is emitted only for a compiler-proven stack/register
        // mapping.  The current CodeView compatibility path does not provide
        // that proof, so no fabricated DW_OP_fbreg location is emitted here.
        for local_name in &function.locals {
            let local = dwarf.unit.add(subprogram, constants::DW_TAG_variable);
            dwarf.unit.get_mut(local).set(
                constants::DW_AT_name,
                AttributeValue::String(local_name.as_bytes().to_vec()),
            );
            dwarf.unit.get_mut(local).set(
                constants::DW_AT_decl_file,
                AttributeValue::FileIndex(Some(file)),
            );
            dwarf.unit.get_mut(local).set(constants::DW_AT_decl_line, AttributeValue::Udata(1));
        }
    }

    let mut write_dwarf = Dwarf::new();
    write_dwarf.units.add(dwarf.unit);
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    write_dwarf.write(&mut sections).map_err(|error| format!("DWARF write failed: {error:?}"))?;
    let mut output = Vec::new();
    sections.for_each(|id, data| {
        if !data.slice().is_empty() {
            output.push((format!("{}", id.name()), data.slice().to_vec()));
        }
        Ok::<(), ()>(())
    }).map_err(|error| format!("DWARF section extraction failed: {error:?}"))?;
    Ok(output)
}

#[allow(dead_code)]
fn _location_expression(offset: i64) -> Expression {
    let mut expression = Expression::new();
    expression.op_fbreg(offset);
    expression
}

#[cfg(test)]
mod tests {
    use super::sections_for_functions;
    use crate::debug::CodeViewFunction;

    #[test]
    fn emits_structural_dwarf_sections_for_real_ranges() {
        let functions = vec![CodeViewFunction {
            name: "helper".to_string(),
            offset: 16,
            size: 32,
            section: 1,
            locals: vec!["debug_value".to_string()],
        }];
        let sections = sections_for_functions("fixture.spectra", "fn helper() {}", &functions)
            .expect("DWARF writer should accept a valid unit");
        assert!(sections.iter().any(|(name, data)| name == ".debug_info" && !data.is_empty()));
        assert!(sections.iter().any(|(name, data)| name == ".debug_line" && !data.is_empty()));
        assert!(sections.iter().any(|(name, data)| name == ".debug_abbrev" && !data.is_empty()));
    }
}
