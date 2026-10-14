import {desktop} from '../../../bridge/desktop';
import type {PackageService} from '../../../bridge/researchPackageTypes';
export const packageService:PackageService={
  preview:desktop.previewResearchPackage,
  export:desktop.exportResearchPackage,
  inspectSelectedFile:desktop.inspectResearchPackageFile,
};
