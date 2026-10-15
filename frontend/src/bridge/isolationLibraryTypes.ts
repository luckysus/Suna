import type {ManagedIsolation} from './managedPackageTypes';

export type IsolationState='stored_unverified'|'requires_original_archive'|'incomplete_or_damaged';
export interface IsolationList {
  schemaVersion:'research-managed-isolations.v1';readOnly:true;writesToLiveWorkspace:false;
  offset:number;total:number;hasMore:boolean;
  items:{isolationId:string;state:IsolationState}[];
}
export interface IsolationView {
  schemaVersion:'research-managed-isolation-view.v1';readOnly:true;
  isolation:ManagedIsolation & {originalArchiveStored:true};
  offset:number;total:number;hasMore:boolean;
  records:{originalId:string;importedId:string;recordSha256:string;record:Record<string,unknown>}[];
}
export interface IsolationLibraryService {
  list(offset:number):Promise<IsolationList>;
  open(isolationId:string,offset:number):Promise<IsolationView>;
}
