//! Generates a minimal AAX page-table XML resource from a processor's
//! parameter list, so `cpp/` stays a purely generic C++ layer with no
//! plugin-specific XML file to hand-maintain.
//!
//! Confirmed empirically (not assumed) that this resource is required: the
//! real AAX Plug-In Validator's `test.parameter_traversal.linear` fails with
//! "Failed to load page tables library" when a plugin registers no
//! `AAX_eResourceType_PageTable` resource at all, even though
//! `test.page_table.load` passes either way (Pro Tools auto-generates a
//! default layout for that path, but the two aren't equivalent).
//!
//! Unlike the rest of this crate's AAX support, this module has no SDK
//! dependency and isn't feature-gated -- it's plain string generation,
//! callable from a plugin's own small build-time binary (see
//! `plugins/gain/src/bin/aax_page_table.rs`) without needing the AAX SDK
//! present.
use gui_host::ParameterInfo;

/// The same plugin-identity FourCC values passed to `aax_entry!` -- kept as
/// plain data here (rather than only inside the generated `extern "C"`
/// getters) so a plugin's page-table-generation binary can reuse them
/// without linking against the AAX SDK.
pub struct AaxPluginIdentity {
    pub manufacturer_id: u32,
    pub product_id: u32,
    pub plugin_id_native: u32,
    pub manufacturer_name: &'static str,
    pub plugin_name: &'static str,
}

fn fourcc_to_str(code: u32) -> String {
    let bytes = [
        (code >> 24) as u8,
        (code >> 16) as u8,
        (code >> 8) as u8,
        code as u8,
    ];
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Renders a minimal, valid page-table XML resource: one page per
/// parameter (in `parameters` order), plus the fixed `MasterBypassID`
/// control every plugin built via `aax_entry!` registers. Deliberately not
/// a full replica of Avid's own multi-console-layout example page tables --
/// just enough structure for the real AAX Plug-In Validator's page-table
/// tests to load and pass (verified against `aaxval`, not assumed).
pub fn generate_page_table_xml(
    identity: &AaxPluginIdentity,
    parameters: &[ParameterInfo],
) -> String {
    let man_id = fourcc_to_str(identity.manufacturer_id);
    let prod_id = fourcc_to_str(identity.product_id);
    let plug_id = fourcc_to_str(identity.plugin_id_native);

    let mut pages = String::new();
    pages.push_str("\t\t\t\t<Page num='1'><ID>MasterBypassID</ID></Page>\n");
    for (i, param) in parameters.iter().enumerate() {
        pages.push_str(&format!(
            "\t\t\t\t<Page num='{}'><ID>p{}</ID></Page>\n",
            i + 2,
            param.id.0
        ));
    }

    format!(
        "<?xml version='1.0' encoding='US-ASCII' standalone='yes'?>\n\
         <PageTables vers='6.4.0.89'>\n\
         \t<PageTableLayouts>\n\
         \t\t<Plugin manID='{man_id}' prodID='{prod_id}' plugID='{plug_id}'>\n\
         \t\t\t<Desc>{name} mkaudio.gui_aax generic layout</Desc>\n\
         \t\t\t<Layout>StandardLayout</Layout>\n\
         \t\t</Plugin>\n\
         \t\t<PTLayout name='StandardLayout'>\n\
         \t\t\t<PageTable type='PgTL' pgsz='1'>\n\
         {pages}\
         \t\t\t</PageTable>\n\
         \t\t</PTLayout>\n\
         \t</PageTableLayouts>\n\
         </PageTables>\n",
        name = identity.plugin_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gui_host::{NormalizedValue, ParameterId};

    #[test]
    fn generates_one_page_per_parameter_plus_bypass() {
        let identity = AaxPluginIdentity {
            manufacturer_id: crate::fourcc(*b"Mkau"),
            product_id: crate::fourcc(*b"Gain"),
            plugin_id_native: crate::fourcc(*b"GnNa"),
            manufacturer_name: "mkaudio",
            plugin_name: "Gain",
        };
        let parameters = [ParameterInfo {
            id: ParameterId(1),
            name: "Gain",
            default_value: NormalizedValue::new(1.0),
            min_value: NormalizedValue::new(0.0),
            max_value: NormalizedValue::new(1.0),
            step_count: None,
        }];

        let xml = generate_page_table_xml(&identity, &parameters);
        assert!(xml.contains("manID='Mkau'"));
        assert!(xml.contains("prodID='Gain'"));
        assert!(xml.contains("plugID='GnNa'"));
        assert!(xml.contains("<ID>MasterBypassID</ID>"));
        assert!(xml.contains("<ID>p1</ID>"));
    }
}
