#!/usr/bin/env python3
"""Convert old define_widget! flat syntax to new impl-block syntax."""

import re
import os
import glob

# Method → trait mapping
METHOD_TRAIT = {
    'preferred_size': 'WidgetLayout',
    'flex_grow': 'WidgetLayout',
    'flex_shrink': 'WidgetLayout',
    'layout_children': 'WidgetLayout',
    'render': 'WidgetRender',
    'post_render': 'WidgetRender',
    'draw_margin': 'WidgetRender',
    'dirty_rect': 'WidgetRender',
    'is_repaint_boundary': 'WidgetRender',
    'children_clip': 'WidgetRender',
    'on_event': 'WidgetEventHandler',
    'needs_continuous_update': 'WidgetEventHandler',
    'scroll_delta': 'WidgetEventHandler',
    'hit_test_frame': 'WidgetEventHandler',
    'hit_test_3d': 'WidgetEventHandler',
    'on_init': 'WidgetLifecycle',
    'on_mount': 'WidgetLifecycle',
    'on_unmount': 'WidgetLifecycle',
    'on_update': 'WidgetLifecycle',
}


def convert_file(path):
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()

    # Skip if already using new syntax
    if 'impl WidgetRender' in content or 'impl WidgetLayout' in content:
        return False

    # Find define_widget! block
    m = re.search(r'(define_widget!\s*\{)(.*?)(^\s*\})', content, re.DOTALL | re.MULTILINE)
    if not m:
        return False

    before = content[:m.start(1)]
    macro_start = m.group(1)
    inner = m.group(2)
    after = content[m.end(3):]

    lines = inner.split('\n')
    
    # Parse: collect struct lines, @new line, and method definitions
    struct_lines = []
    new_line = None
    new_rest = []
    methods = []  # [(method_name, full_line_plus_body)]
    
    i = 0
    phase = 'struct'  # struct → new → methods → done
    
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        
        if phase == 'struct':
            if stripped.startswith('@new'):
                phase = 'new'
                new_line = line
                i += 1
                continue
            elif re.match(r'^\s*\w+\s*=>', stripped):
                phase = 'methods'
                continue
            else:
                struct_lines.append(line)
                i += 1
                continue
        
        if phase == 'new':
            if re.match(r'^\s*\w+\s*=>', stripped):
                phase = 'methods'
                continue
            else:
                new_rest.append(line)
                i += 1
                continue
        
        if phase == 'methods':
            if re.match(r'^\s*(\w+)\s*=>', stripped):
                method_name = re.match(r'^\s*(\w+)\s*=>', stripped).group(1)
                if method_name in METHOD_TRAIT:
                    # Collect this method + its body
                    method_lines = [line]
                    brace_count = stripped.count('{') - stripped.count('}')
                    j = i + 1
                    while j < len(lines) and brace_count > 0:
                        l = lines[j]
                        method_lines.append(l)
                        brace_count += l.count('{') - l.count('}')
                        j += 1
                    methods.append((method_name, method_lines))
                    i = j
                    continue
            
            # Check if it's the closing brace
            if stripped == '}':
                phase = 'done'
                continue
            # Skip non-method lines (like comments between methods)
            i += 1
            continue
        
        i += 1

    if not methods:
        return False  # No methods to convert
    
    # Regroup methods by trait
    trait_methods = {}
    for mname, mlines in methods:
        trait = METHOD_TRAIT[mname]
        if trait not in trait_methods:
            trait_methods[trait] = []
        trait_methods[trait].extend(mlines)
        trait_methods[trait].append('')  # blank line between methods
    
    # Build new inner content
    new_inner_lines = list(struct_lines)
    
    # @new
    if new_line is not None:
        new_inner_lines.append(new_line)
        new_inner_lines.extend(new_rest)
    
    # Impl blocks (sorted for determinism)
    for trait in sorted(trait_methods.keys(), key=lambda t: ['WidgetLayout','WidgetRender','WidgetEventHandler','WidgetLifecycle'].index(t)):
        new_inner_lines.append(f'    impl {trait} {{')
        for ml in trait_methods[trait]:
            if ml.strip():
                new_inner_lines.append(ml)
        new_inner_lines.append('    }')
    
    # Remove trailing empty lines before closing brace
    while new_inner_lines and new_inner_lines[-1].strip() == '':
        new_inner_lines.pop()
    
    new_inner = '\n'.join(new_inner_lines)
    
    # Assemble
    result = before + macro_start + new_inner + '\n' + after
    
    with open(path, 'w', encoding='utf-8') as f:
        f.write(result)
    
    return True


def main():
    ui_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'ui', 'src')
    count = 0
    for root, dirs, files in os.walk(ui_dir):
        for f in files:
            if f.endswith('.rs'):
                path = os.path.join(root, f)
                if convert_file(path):
                    print(f'  Converted: {os.path.relpath(path, ui_dir)}')
                    count += 1
    print(f'Done: {count} files converted')


if __name__ == '__main__':
    main()
