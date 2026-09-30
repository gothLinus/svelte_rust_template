use std::{collections::BTreeSet, sync::Arc};

use fluent_bundle::FluentResource;
use fluent_syntax::ast::{
    CallArguments, Entry, Expression, InlineExpression, Pattern, PatternElement,
};

type Resources = [(String, Arc<FluentResource>)];

pub(crate) fn ids(resources: &Resources) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for (_, resource) in resources {
        for entry in resource.entries() {
            if let Entry::Message(message) = entry {
                ids.insert(message.id.name.to_owned());
            }
        }
    }
    ids.into_iter().collect()
}

/// Variables read by message `id`, including through its attributes, sorted.
pub(crate) fn variables(resources: &Resources, id: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    for (_, resource) in resources {
        for entry in resource.entries() {
            if let Entry::Message(message) = entry
                && message.id.name == id
            {
                if let Some(pattern) = &message.value {
                    pattern_variables(pattern, &mut found);
                }
                for attribute in &message.attributes {
                    pattern_variables(&attribute.value, &mut found);
                }
            }
        }
    }
    found.into_iter().collect()
}

fn pattern_variables(pattern: &Pattern<&str>, found: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            expression_variables(expression, found);
        }
    }
}

fn expression_variables(expression: &Expression<&str>, found: &mut BTreeSet<String>) {
    match expression {
        Expression::Inline(inline) => inline_variables(inline, found),
        Expression::Select { selector, variants } => {
            inline_variables(selector, found);
            for variant in variants {
                pattern_variables(&variant.value, found);
            }
        }
    }
}

fn inline_variables(inline: &InlineExpression<&str>, found: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            found.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            argument_variables(arguments, found);
        }
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => argument_variables(arguments, found),
        InlineExpression::Placeable { expression } => expression_variables(expression, found),
        InlineExpression::MessageReference { .. }
        | InlineExpression::TermReference { .. }
        | InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. } => {}
    }
}

fn argument_variables(arguments: &CallArguments<&str>, found: &mut BTreeSet<String>) {
    for positional in &arguments.positional {
        inline_variables(positional, found);
    }
    for named in &arguments.named {
        inline_variables(&named.value, found);
    }
}
