import {
  Terminal,
  TerminalSquare,
  Monitor,
  MonitorSmartphone,
  FolderUp,
  FolderLock,
  Cable,
  FileText,
  Globe,
  Smartphone,
  Cloud,
  Boxes,
  Server,
  Network,
  Plug,
} from "@lucide/svelte";

export interface KindInfo {
  id: string;
  label: string;
  description: string;
  icon: any;
  defaultPort: number;
  /** Whether Cablebag can open this session type yet. */
  ready: boolean;
  needsHost: boolean;
  /** Not offered for new sessions; imported ones stay saved. */
  hidden?: boolean;
}

export const KINDS: KindInfo[] = [
  { id: "ssh", label: "SSH", description: "Secure shell with file browser", icon: Terminal, defaultPort: 22, ready: true, needsHost: true },
  { id: "rdp", label: "RDP", description: "Windows remote desktop", icon: Monitor, defaultPort: 3389, ready: true, needsHost: true },
  { id: "vnc", label: "VNC", description: "Remote screen", icon: MonitorSmartphone, defaultPort: 5900, ready: true, needsHost: true },
  { id: "telnet", label: "Telnet", description: "Unencrypted terminal", icon: Plug, defaultPort: 23, ready: true, needsHost: true },
  { id: "sftp", label: "SFTP", description: "Remote files only", icon: FolderLock, defaultPort: 22, ready: true, needsHost: true },
  { id: "ftp", label: "FTP", description: "File transfer, with optional TLS", icon: FolderUp, defaultPort: 21, ready: true, needsHost: true },
  { id: "serial", label: "Serial", description: "COM port of a switch or other device", icon: Cable, defaultPort: 0, ready: true, needsHost: false },
  { id: "shell", label: "Local shell", description: "PowerShell, cmd, Git Bash", icon: TerminalSquare, defaultPort: 0, ready: true, needsHost: false },
  { id: "wsl", label: "WSL", description: "Linux distribution on this PC", icon: Boxes, defaultPort: 0, ready: true, needsHost: false },
  { id: "mosh", label: "Mosh", description: "Mobile shell over UDP", icon: Smartphone, defaultPort: 22, ready: false, hidden: true, needsHost: true },
  { id: "rsh", label: "Rsh", description: "Legacy remote shell", icon: Server, defaultPort: 514, ready: false, hidden: true, needsHost: true },
  { id: "xdmcp", label: "XDMCP", description: "Remote Unix desktop", icon: Network, defaultPort: 177, ready: false, hidden: true, needsHost: true },
  { id: "browser", label: "Browser", description: "Web page in a tab", icon: Globe, defaultPort: 0, ready: true, needsHost: false },
  { id: "file", label: "File or folder", description: "Open a path or link", icon: FileText, defaultPort: 0, ready: true, needsHost: false },
  { id: "s3", label: "AWS S3", description: "Bucket browser", icon: Cloud, defaultPort: 0, ready: false, hidden: true, needsHost: false },
];

export const kindInfo = (id: string): KindInfo =>
  KINDS.find((k) => k.id === id) ?? { id, label: id, description: "", icon: Server, defaultPort: 0, ready: false, needsHost: true };
