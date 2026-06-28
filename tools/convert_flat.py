#!/usr/bin/env python3
"""Convert flat method syntax to grouped impl-block syntax."""
import re, sys, os

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

def convert_file(path):
    with open(path,'r',encoding='utf-8') as f:
        content = f.read()
    # Check if has flat methods (no impl blocks after define_widget)
    if 'define_widget!' not in content: return False
    if 'impl WidgetLayout' in content: return False
    
    # Find define_widget! block start
    idx = content.find('define_widget!')
    if idx < 0: return False
    
    # Count braces to find end
    brace_start = content.find('{', idx)
    if brace_start < 0: return False
    depth = 0
    block_end = brace_start
    for i in range(brace_start, len(content)):
        if content[i] == '{': depth += 1
        elif content[i] == '}': depth -= 1
        if depth == 0: block_end = i; break
    
    inner = content[brace_start+1:block_end]
    
    # Find @new and method lines
    lines = inner.split('\n')
    struct_lines = []
    new_line = None
    new_rest = []
    methods = {}
    current_method = None
    current_lines = []
    in_new = False
    in_methods = False
    
    for line in lines:
        s = line.strip()
        if not in_methods and not in_new:
            if s.startswith('@new'):
                in_new = True
                new_line = line
                continue
            m = re.match(r'^\s*(\w+)\s*=>', s)
            if m and m.group(1) in METHOD_TRAIT:
                in_methods = True
                current_method = m.group(1)
                current_lines = [line]
                continue
            struct_lines.append(line)
        elif in_new:
            m = re.match(r'^\s*(\w+)\s*=>', s)
            if m and m.group(1) in METHOD_TRAIT:
                in_new = False
                in_methods = True
                current_method = m.group(1)
                current_lines = [line]
                continue
            new_rest.append(line)
        elif in_methods:
            m = re.match(r'^\s*(\w+)\s*=>', s)
            if m and m.group(1) in METHOD_TRAIT:
                if current_method:
                    methods.setdefault(METHOD_TRAIT[current_method],[]).extend(current_lines)
                    methods[METHOD_TRAIT[current_method]].append('')
                current_method = m.group(1)
                current_lines = [line]
                continue
            current_lines.append(line)
    
    if current_method:
        methods.setdefault(METHOD_TRAIT[current_method],[]).extend(current_lines)
    
    if not methods: return False
    
    # Build new inner
    new_inner = list(struct_lines)
    if new_line is not None:
        new_inner.append(new_line)
        new_inner.extend(new_rest)
    
    trait_order = ['WidgetLayout','WidgetRender','WidgetEventHandler','WidgetLifecycle']
    for trait in trait_order:
        if trait in methods:
            ml = methods[trait]
            while ml and ml[-1].strip() == '': ml.pop()
            new_inner.append(f'    impl {trait} {{')
            for l in ml:
                new_inner.append(l)
            new_inner.append('    }')
    
    new_inner_str = '\n'.join(new_inner)
    result = content[:brace_start+1] + '\n' + new_inner_str + '\n' + content[block_end:]
    
    with open(path,'w',encoding='utf-8') as f:
        f.write(result)
    return True

base = os.path.join(os.path.dirname(__file__), '..', 'ui', 'src')
for root, dirs, files in os.walk(base):
    for f in files:
        if f.endswith('.rs'):
            path = os.path.join(root, f)
            try:
                if convert_file(path):
                    print(f'  {os.path.relpath(path, base)}')
            except:
                pass
print('Done')
