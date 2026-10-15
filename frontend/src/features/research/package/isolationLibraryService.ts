import {desktop} from '../../../bridge/desktop';
import type {IsolationLibraryService} from '../../../bridge/isolationLibraryTypes';
export const isolationLibraryService:IsolationLibraryService={
  list:desktop.listResearchPackageIsolations,
  open:desktop.openResearchPackageIsolation,
};
