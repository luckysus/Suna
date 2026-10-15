import {desktop} from '../../../bridge/desktop';
import type {ManagedService} from '../../../bridge/managedPackageTypes';
export const managedService:ManagedService={
  inventory:desktop.listResearchManagedAssets,
  preview:desktop.previewResearchManagedPackage,
  export:desktop.exportResearchManagedPackage,
  inspect:desktop.inspectResearchManagedPackageFile,
  importIsolated:desktop.importResearchManagedPackageIsolated,
};
