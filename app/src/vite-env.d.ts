interface ImportMetaEnv {
  readonly VITE_MOCK?: string;
  readonly VITE_FABRIC_ROOT?: string;
  readonly VITE_FABRIC_HOME?: string;
}
interface ImportMeta {
  readonly env: ImportMetaEnv;
}
