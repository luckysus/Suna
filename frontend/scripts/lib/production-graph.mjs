import {existsSync,readFileSync} from 'node:fs';
import {dirname,extname,join,resolve} from 'node:path';

export function relativeImports(source){
  const staticImports=/(?:import|export)\s+(?:[^"']*?\sfrom\s*)?["'](\.[^"']+)["']/g;
  const deferredImports=/\bimport\s*\(\s*["'](\.[^"']+)["']\s*\)/g;
  return [...new Set([...source.matchAll(staticImports),...source.matchAll(deferredImports)].map(m=>m[1]))];
}
function localModule(from,specifier){
  const base=resolve(dirname(from),specifier);
  return [base,`${base}.ts`,`${base}.tsx`,join(base,'index.ts'),join(base,'index.tsx')]
    .find(candidate=>existsSync(candidate)&&['.ts','.tsx'].includes(extname(candidate)));
}
export function collectProductionFiles(entry){
  const seen=new Set(),files=[];
  function visit(path){
    if(seen.has(path))return;seen.add(path);files.push(path);
    for(const specifier of relativeImports(readFileSync(path,'utf8'))){
      const dependency=localModule(path,specifier);if(dependency)visit(dependency);
    }
  }
  visit(resolve(entry));return files;
}
