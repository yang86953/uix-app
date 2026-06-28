#!/usr/bin/env python3
"""Convert ALL define_widget! invocations to grouped impl-block syntax."""
import re, os

METHOD_TRAIT = {
    'preferred_size':'WidgetLayout','flex_grow':'WidgetLayout',
    'flex_shrink':'WidgetLayout','layout_children':'WidgetLayout',
    'render':'WidgetRender','post_render':'WidgetRender',
    'draw_margin':'WidgetRender','dirty_rect':'WidgetRender',
    'is_repaint_boundary':'WidgetRender','children_clip':'WidgetRender',
    'on_event':'WidgetEventHandler','needs_continuous_update':'WidgetEventHandler',
    'scroll_delta':'WidgetEventHandler','hit_test_frame':'WidgetEventHandler',
    'hit_test_3d':'WidgetEventHandler',
    'on_init':'WidgetLifecycle','on_mount':'WidgetLifecycle',
    'on_unmount':'WidgetLifecycle','on_update':'WidgetLifecycle',
}

def find_macro_block(content, start):
    idx = content.find('define_widget!', start)
    if idx < 0: return None
    brace_start = content.find('{', idx)
    if brace_start < 0: return None
    depth = 0
    block_end = brace_start
    for i in range(brace_start, len(content)):
        if content[i] == '{': depth += 1
        elif content[i] == '}': depth -= 1
        if depth == 0: block_end = i; break
    return (content[:brace_start+1], content[brace_start+1:block_end], content[block_end:])

def convert_block(inner):
    lines = inner.split('\n')
    if any('impl WidgetLayout' in l or 'impl WidgetRender' in l for l in lines):
        return None
    
    struct_lines = []
    new_line = None
    new_rest = []
    trait_methods = {}  # trait_name -> [lines]
    flat_lines = []     # lines not in any trait (build, visible)
    current_lines = []
    current_trait = None
    in_new = False
    in_methods = False
    
    def flush_method():
        nonlocal current_trait, current_lines
        if current_trait and current_lines:
            trait_methods.setdefault(current_trait, []).extend(current_lines)
            trait_methods[current_trait].append('')
        elif current_lines:
            flat_lines.extend(current_lines)
            flat_lines.append('')
        current_trait = None
        current_lines = []
    
    for line in lines:
        s = line.strip()
        if not in_methods and not in_new:
            if s.startswith('@new'):
                in_new = True
                new_line = line
                continue
            m = re.match(r'^(\w+)\s*=>', s)
            if m:
                in_methods = True
                mname = m.group(1)
                if mname in METHOD_TRAIT:
                    flush_method()
                    current_trait = METHOD_TRAIT[mname]
                    current_lines = [line]
                else:
                    # Not in any trait (build, visible) — keep as flat
                    flush_method()
                    current_lines = [line]
                continue
            struct_lines.append(line)
        elif in_new:
            m = re.match(r'^(\w+)\s*=>', s)
            if m:
                in_new = False
                in_methods = True
                mname = m.group(1)
                if mname in METHOD_TRAIT:
                    flush_method()
                    current_trait = METHOD_TRAIT[mname]
                    current_lines = [line]
                else:
                    flush_method()
                    current_lines = [line]
                continue
            new_rest.append(line)
        elif in_methods:
            m = re.match(r'^(\w+)\s*=>', s)
            if m:
                mname = m.group(1)
                if mname in METHOD_TRAIT:
                    flush_method()
                    current_trait = METHOD_TRAIT[mname]
                    current_lines = [line]
                else:
                    flush_method()
                    current_lines = [line]
                continue
            current_lines.append(line)
    
    flush_method()
    
    if not trait_methods:
        return None
    
    new_inner = list(struct_lines)
    if new_line is not None:
        new_inner.append(new_line)
        new_inner.extend(new_rest)
    
    trait_order = ['WidgetLayout','WidgetRender','WidgetEventHandler','WidgetLifecycle']
    for trait in trait_order:
        if trait in trait_methods:
            ml = trait_methods[trait]
            while ml and ml[-1].strip() == '': ml.pop()
            new_inner.append('    impl ' + trait + ' {')
            for l in ml:
                new_inner.append(l)
            new_inner.append('    }')
    
    # Flat lines (build, visible) → 转注释（新语法不支持平铺方法）
    if flat_lines:
        for l in flat_lines:
            if l.strip():
                new_inner.append('    // ' + l.strip() + '  // 已忽略：build/visible 使用 trait 默认值')
    
    return '\n'.join(new_inner)

def convert_file(path):
    with open(path,'r',encoding='utf-8') as f:
        content = f.read()
    if 'define_widget!' not in content:
        return False
    pos = 0
    changed = False
    while True:
        result = find_macro_block(content, pos)
        if result is None:
            break
        before, inner, after = result
        new_inner = convert_block(inner)
        if new_inner is not None:
            content = before + '\n' + new_inner + '\n' + after
            changed = True
            pos = len(before) + len(new_inner) + 2
        else:
            pos = len(before) + len(inner) + 2
    if changed:
        with open(path,'w',encoding='utf-8') as f:
            f.write(content)
        return True
    return False

base = os.path.join(os.path.dirname(__file__), '..', 'ui', 'src')
count = 0
for root, dirs, files in os.walk(base):
    for f in files:
        if f.endswith('.rs'):
            path = os.path.join(root, f)
            try:
                if convert_file(path):
                    print(f'  {os.path.relpath(path, base)}')
                    count += 1
            except Exception as e:
                print(f'  ERROR {os.path.relpath(path, base)}: {e}')
print(f'Done: {count} files')
