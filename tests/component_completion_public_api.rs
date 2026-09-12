//! 光标所在的部分名称使用真实检查前缀；失败事实不能作为生成/运行产物。
#![cfg(feature = "lang-build")]
use std::collections::BTreeMap;
use uix_app::lang::compiler::component_source::{completion::*, *};
use uix_app::lang::runtime::Type as DataType;

fn complete(marked: &str) -> Completion {
    let offset = marked.find('§').unwrap();
    let source = marked.replace('§', "");
    let libraries = BTreeMap::from([(
        "native".into(),
        BTreeMap::from([(
            "Text".into(),
            NativeExport::Component(ComponentSignature {
                parameters: vec![("children".into(), Type::Data(DataType::String))],
                required: ["children".into()].into(),
            }),
        )]),
    )]);
    let inspection = inspect_inline(&source, "untitled:completion", &libraries).unwrap();
    inspection
        .complete(inspection.source().source_graph.root(), offset)
        .unwrap()
}
fn labels(output: &Completion) -> Vec<&str> {
    output
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect()
}

#[test]
fn default_binding_order_forward_methods_and_state_prefix_match_real_checker() {
    let output = complete("export component Main(first:Int=1, second:Int=fi§){return <></>;}");
    assert_eq!(labels(&output), ["first"]);
    assert!(output.is_incomplete);
    let output = complete(
        "function future():Int{return 1;} export component Main(future:Int=fu§){return <></>;}",
    );
    assert!(output.items.is_empty());
    let output = complete(
        "export component Main(){let action=lat§; function later():Int{return 1;} return <></>;}",
    );
    assert_eq!(labels(&output), ["later"]);
    let output = complete(
        "export component Main(){state value:Int=lat§; function later():Int{return 1;} return <></>;}",
    );
    assert!(output.items.is_empty());
}

#[test]
fn branches_initializers_and_captures_do_not_leak_future_or_shadowed_bindings() {
    let inside = complete(
        "export component Main(input:Int=1){if(true){let inside=2;let result=in§;}return <></>;}",
    );
    assert_eq!(labels(&inside), ["input", "inside"]);
    let after = complete(
        "export component Main(input:Int=1){if(true){let inside=2;}let result=in§;return <></>;}",
    );
    assert_eq!(labels(&after), ["input"]);
    let initializer =
        complete("export component Main(input:Int=1){if(true){let input=in§;}return <></>;}");
    assert_eq!(labels(&initializer), ["input"]);
    let item = &initializer.items[0];
    let Some(symbols::SymbolTarget::Binding(id)) = item.target else {
        panic!();
    };
    assert_eq!(id.start, "export component Main(".len());
    let future = complete("export component Main(){let result=lat§;let later=1;return <></>;}");
    assert!(future.items.is_empty());
    let capture = complete(
        "export component Main(){let before=1;function read():Int{return aft§;}let after=2;return <></>;}",
    );
    assert!(capture.items.is_empty());
    let lambda = complete(
        "export component Main(input:Int=1){let outer=2;let result=[1].map((input)=>in§);return <></>;}",
    );
    assert_eq!(labels(&lambda), ["input"]);
    let Some(symbols::SymbolTarget::Binding(id)) = lambda.items[0].target else {
        panic!();
    };
    assert!(id.start > "export component Main(input:Int=1){let outer=2;".len());
}

#[test]
fn context_selects_types_tags_properties_and_receiver_members_not_global_word_lists() {
    let types =
        complete("type Caption=String; export component Main(){let value:Cap§=\"\";return <></>;}");
    assert_eq!(labels(&types), ["Caption"]);
    let members = complete(
        "import {Text} from 'native'; export component Main(label:String=\"文字😀\"){return <Text>{label.toU§()}</Text>;}",
    );
    assert_eq!(labels(&members), ["toUpperCase"]);
    let fields = complete(
        "export component Main(){let record={alpha:1,beta:2};let result=record.al§;return <></>;}",
    );
    assert_eq!(labels(&fields), ["alpha"]);
    let props = complete(
        "component Card(title:String,titleExtra:String=\"\"){return <></>;} export component Main(){return <Card title=\"现有\" ti§=\"\"/>;}",
    );
    assert_eq!(labels(&props), ["titleExtra"]);
    let tags = complete("component Card(){return <></>;} export component Main(){return <Ca§/>;}");
    assert_eq!(labels(&tags), ["Card"]);
    let shadowed = complete(
        "component Card(){return <></>;} export component Main(){let Card=1;return <Ca§/>;}",
    );
    assert!(shadowed.items.is_empty());
    // 已核对的开始标签不因为后续缺少必需属性而丢失补全。
    let missing = complete(
        "component Card(title:String,extra:String){return <></>;} export component Main(){return <Ca§rd/>;}",
    );
    assert_eq!(labels(&missing), ["Card"]);
    let missing = complete(
        "component Card(title:String,extra:String){return <></>;} export component Main(){return <Card ti§tle=\"\"/>;}",
    );
    assert_eq!(labels(&missing), ["title"]);
    let missing = complete(
        "component Card(title:String){return <></>;} export component Main(){return <Card §/>;}",
    );
    assert_eq!(labels(&missing), ["key", "title"]);
}

#[test]
fn comments_literals_and_raw_text_do_not_receive_code_completions() {
    for source in [
        "export component Main(input:Int=1){/* inp§ */return <></>;}",
        "export component Main(input:Int=1){let value=\"inp§\";return <></>;}",
        "import {Text} from 'native';export component Main(input:Int=1){return <Text>inp§</Text>;}",
    ] {
        assert!(complete(source).items.is_empty(), "{source}");
    }
    let output = complete("export component Main(input:Int=1){let result=in§put;return <></>;}");
    assert_eq!(labels(&output), ["input"]);
    assert_eq!(output.replacement.end - output.replacement.start, 5);
    assert!(!output.is_incomplete);
}

#[test]
fn deep_partial_scopes_and_candidate_caps_stay_bounded_without_fake_success() {
    let source = format!(
        "function global():Int{{return 1;}} export component Main(){{let nested={}glo§;return <></>;}}",
        "()=>".repeat(50)
    );
    assert_eq!(labels(&complete(&source)), ["global"]);
    let valid = source.replace("glo§", "global()");
    assert!(check(link_inline(&valid, "deep.uix").unwrap(), &BTreeMap::new()).is_ok());
    let too_deep = source
        .replace(&"()=>".repeat(50), &"()=>".repeat(100))
        .replace('§', "");
    assert_eq!(
        inspect_inline(&too_deep, "deep.uix", &BTreeMap::new())
            .unwrap_err()
            .code,
        "component-depth-limit"
    );
    let declarations = (0..300)
        .map(|number| format!("let value_{number}={number};"))
        .collect::<String>();
    let source =
        format!("export component Main(){{{declarations}let read=val§ue_0;return <></>;}}");
    let result = complete(&source);
    assert_eq!(result.items.len(), 256);
    assert!(result.is_incomplete);
    let source = "export component Main(input:Int=1){let value=bad;let read=in§;return <></>;}";
    // 更早错误之后尚未遍历的范围不能借父 owner 的已知变量冒充完整作用域。
    assert!(complete(source).items.is_empty());
    assert_eq!(
        labels(&declaration_completion("/* 中文😀 */ com", "/* 中文😀 */ com".len()).unwrap()),
        ["component"]
    );
    assert!(declaration_completion("/* unfinished com", 17).is_none());
    let comments = "/* */ ".repeat(66_000);
    let source =
        format!("export component Main(input:Int=1){{{comments}let read=input;return <></>;}}");
    let error = inspect_inline(&source, "comments.uix", &BTreeMap::new()).unwrap_err();
    assert_eq!(error.code, "component-token-limit");
}

#[test]
fn missing_structure_preserves_original_positions_and_never_becomes_checked_output() {
    for marked in [
        "export component Main(input:Int=1){let value=in§",
        "export component Main(input:Int=1){let value=§",
        "export component Main(input:Int=1){let value=(in§",
        "export component Main(input:Int=1){let value=[in§",
        "export component Main(input:Int=1){if(true){let value=in§",
        "export component Main(input:Int=1){let value=§;return <></>;}",
        "export component Main(input:Int=1){let value=(in§;return <></>;}",
        "export component Main(input:Int=1){let value=[in§;return <></>;}",
    ] {
        let offset = marked.find('§').unwrap();
        let source = marked.replace('§', "");
        let inspection = inspect_inline(&source, "incomplete.uix", &BTreeMap::new()).unwrap();
        assert!(inspection.diagnostic().is_some(), "{marked}");
        assert!(parse(&source, "incomplete.uix").is_err(), "{marked}");
        assert!(
            check(inspection.source().clone(), &BTreeMap::new()).is_err(),
            "{marked}"
        );
        let file = inspection.source().source_graph.root();
        assert_eq!(
            inspection.source().source_graph.file(file).unwrap().source,
            source
        );
        let output = inspection.complete(file, offset).unwrap();
        assert!(
            labels(&output).contains(&"input"),
            "{marked}: {:?}",
            labels(&output)
        );
        assert!(output.is_incomplete);
        assert!(output.replacement.end <= source.len());
    }
    let output = complete("export component Main(input:Int=1){let broken=;let value=in§");
    assert!(
        output.items.is_empty(),
        "Earlier missing expression must not validate later scopes"
    );
}

#[test]
fn unfinished_members_types_tags_attributes_and_callbacks_use_existing_contracts() {
    let members = complete("export component Main(label:String=\"😀\"){let value=label.§");
    assert!(labels(&members).contains(&"toUpperCase"));
    let fields =
        complete("export component Main(){let record={alpha:1};let value=record.§;return <></>;}");
    assert_eq!(labels(&fields), ["alpha"]);
    let types = complete("type Caption=String;export component Main(){let value:§");
    assert!(labels(&types).contains(&"Caption"));
    assert!(labels(&types).contains(&"String"));
    let types = complete("type Caption=String;function read():§");
    assert!(labels(&types).contains(&"Caption"));
    let close =
        complete("component Card(){return <></>;}export component Main(){return <Card></Ca§");
    assert_eq!(labels(&close), ["Card"]);
    for tag in ["<Ca§", "<§"] {
        let source =
            format!("component Card(){{return <></>;}}export component Main(){{return {tag}");
        assert_eq!(
            labels(&complete(&source)),
            ["Card", "Main"]
                .into_iter()
                .filter(|name| tag == "<§" || name.starts_with("Ca"))
                .collect::<Vec<_>>()
        );
    }
    let props = complete(
        "component Card(title:String){return <></>;}export component Main(){return <Card §",
    );
    assert_eq!(labels(&props), ["key", "title"]);
    let value = complete(
        "component Card(title:String){return <></>;}export component Main(label:String=\"\"){return <Card title={la§",
    );
    assert_eq!(labels(&value), ["label"]);
    let value = complete(
        "component Card(title:String){return <></>;}export component Main(label:String=\"\"){return <Card title=§",
    );
    assert!(
        value.items.is_empty(),
        "Bare attribute values do not accept variable syntax"
    );
    let lambda = complete("export component Main(input:Int=1){let value=[1].map((item)=>it§");
    assert_eq!(labels(&lambda), ["item"]);
}

#[test]
fn recovery_keeps_limits_and_literal_regions_and_does_not_hide_semantic_failures() {
    let fifty = format!(
        "export component Main(input:Int=1){{let value={}in§",
        "()=>".repeat(50)
    );
    assert_eq!(labels(&complete(&fifty)), ["input"]);
    let source = format!("export component Main(){{return {}", "<><>".repeat(30));
    assert_eq!(
        inspect_inline(&source, "limits.uix", &BTreeMap::new())
            .unwrap_err()
            .code,
        "component-recovery-limit"
    );
    for source in [
        "import {Text} from 'native';export component Main(input:Int=1){return <Text>in§",
        "export component Main(input:Int=1){let value=\"in§\"",
    ] {
        assert!(complete(source).items.is_empty(), "{source}");
    }
    // Missing final structure must not hide an earlier real error and make later values visible.
    let value = complete("export component Main(input:Int=1){let value=unknown;let next=in§");
    assert!(value.items.is_empty());
    let value = complete(
        "export component Main(){state count:Int=0;function change():Unit{count=count+1;}let next=cha§",
    );
    assert_eq!(labels(&value), ["change"]);
    assert!(value.items[0].detail.contains("effect: not yet checked"));
    let value = complete("export component Main(){let callback=()=>1;let next=call§");
    assert_eq!(labels(&value), ["callback"]);
    assert!(value.items[0].detail.contains("effect: not yet checked"));
}

#[test]
fn editor_truncations_and_interior_deletions_are_bounded_and_preserve_strict_rejection() {
    let fixture = "type Label=String; component Card(title:Label){return <></>;}export component Main(input:String=\"中文😀\"){state count:Int=1;let read=[1,2].map((item)=>item+count);if(true){let local={name:input};return <Card title={local.name}/>;}return <></>; }";
    let variants = fixture
        .char_indices()
        .map(|(at, _)| fixture[..at].to_owned())
        .chain(
            fixture
                .char_indices()
                .step_by(3)
                .map(|(at, ch)| format!("{}{}", &fixture[..at], &fixture[at + ch.len_utf8()..])),
        );
    for source in variants {
        let boundary = |error: &uix_app::lang::compiler::CompilerDiagnostic| {
            assert!(
                error.start <= error.end && error.end <= source.len(),
                "{source}"
            );
            assert!(source.is_char_boundary(error.start) && source.is_char_boundary(error.end));
        };
        match inspect_inline(&source, "edits.uix", &BTreeMap::new()) {
            Ok(inspection) => {
                if let Some(error) = inspection.diagnostic() {
                    boundary(error);
                }
                let result = inspection
                    .complete(inspection.source().source_graph.root(), source.len())
                    .unwrap();
                assert!(result.items.len() <= 256);
                if parse(&source, "edits.uix").is_err() {
                    assert!(inspection.diagnostic().is_some());
                    assert!(check(inspection.source().clone(), &BTreeMap::new()).is_err());
                }
            }
            Err(error) => boundary(&error),
        }
    }
}
