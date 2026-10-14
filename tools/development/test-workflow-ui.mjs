import {createRequire} from "node:module";
import {existsSync,lstatSync,realpathSync,symlinkSync} from "node:fs";
import {dirname,join} from "node:path";
import {fileURLToPath} from "node:url";
import {spawnSync} from "node:child_process";

const root=join(dirname(fileURLToPath(import.meta.url)),"../..");
const frontend=join(root,"frontend"),lab=join(root,"tools/development/research-workflow-ui");
const modules=join(frontend,"node_modules"),link=join(lab,"node_modules");
if(!existsSync(join(modules,"vitest/package.json")))throw new Error("Install the locked frontend environment first");
if(!existsSync(link))symlinkSync(modules,link,process.platform==="win32"?"junction":"dir");
else if(!lstatSync(link).isSymbolicLink()||realpathSync(link)!==realpathSync(modules))throw new Error("Existing lab dependency directory is not the shared frontend link; no files were overwritten");
const requireFrontend=createRequire(join(frontend,"package.json"));
for(const [script,args] of [[join(modules,"vitest/vitest.mjs"),["run"]],[requireFrontend.resolve("typescript/bin/tsc"),["--noEmit"]]]){
  const result=spawnSync(process.execPath,[script,...args],{cwd:lab,stdio:"inherit"});
  if(result.error)throw new Error("Development UI validation could not start");
  if(result.status!==0)process.exit(result.status??1);
}
