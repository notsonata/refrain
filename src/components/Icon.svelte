<script lang="ts">
  import Activity from '~icons/lucide/activity';
  import ArrowDownNarrowWide from '~icons/lucide/arrow-down-narrow-wide';
  import ArrowUpNarrowWide from '~icons/lucide/arrow-up-narrow-wide';
  import ChevronDown from '~icons/lucide/chevron-down';
  import ChevronLeft from '~icons/lucide/chevron-left';
  import ChevronRight from '~icons/lucide/chevron-right';
  import Check from '~icons/lucide/check';
  import CloudDownload from '~icons/lucide/cloud-download';
  import Copy from '~icons/lucide/copy';
  import Database from '~icons/lucide/database';
  import Disc3 from '~icons/lucide/disc-3';
  import Download from '~icons/lucide/download';
  import Ellipsis from '~icons/lucide/ellipsis';
  import ExternalLink from '~icons/lucide/external-link';
  import FolderOpen from '~icons/lucide/folder-open';
  import GripVertical from '~icons/lucide/grip-vertical';
  import HardDrive from '~icons/lucide/hard-drive';
  import Heart from '~icons/lucide/heart';
  import Info from '~icons/lucide/info';
  import ListMusic from '~icons/lucide/list-music';
  import Plus from '~icons/lucide/plus';
  import RefreshCw from '~icons/lucide/refresh-cw';
  import Repeat2 from '~icons/lucide/repeat-2';
  import Search from '~icons/lucide/search';
  import Settings from '~icons/lucide/settings';
  import SlidersHorizontal from '~icons/lucide/sliders-horizontal';
  import TriangleAlert from '~icons/lucide/triangle-alert';
  import X from '~icons/lucide/x';
  import Spotify from '~icons/simple-icons/spotify';

  type IconName =
    | 'local'
    | 'spotify'
    | 'heart'
    | 'album'
    | 'playlist'
    | 'settings'
    | 'issues'
    | 'search'
    | 'refresh'
    | 'filter'
    | 'chevron-down'
    | 'chevron-right'
    | 'back'
    | 'check'
    | 'plus'
    | 'cloud'
    | 'copy'
    | 'warning'
    | 'close'
    | 'more'
    | 'folder'
    | 'download'
    | 'sliders'
    | 'sort-asc'
    | 'sort-desc'
    | 'grip'
    | 'sync'
    | 'info'
    | 'database'
    | 'link'
    | 'activity';

  export let name: IconName;
  export let size = 18;
  export let strokeWidth = 1.8;

  const icons = {
    local: HardDrive,
    spotify: Spotify,
    heart: Heart,
    album: Disc3,
    playlist: ListMusic,
    settings: Settings,
    issues: TriangleAlert,
    search: Search,
    refresh: RefreshCw,
    filter: SlidersHorizontal,
    'chevron-down': ChevronDown,
    'chevron-right': ChevronRight,
    back: ChevronLeft,
    check: Check,
    plus: Plus,
    cloud: CloudDownload,
    copy: Copy,
    warning: TriangleAlert,
    close: X,
    more: Ellipsis,
    folder: FolderOpen,
    download: Download,
    sliders: SlidersHorizontal,
    'sort-asc': ArrowUpNarrowWide,
    'sort-desc': ArrowDownNarrowWide,
    grip: GripVertical,
    sync: Repeat2,
    info: Info,
    database: Database,
    link: ExternalLink,
    activity: Activity,
  } as const;

  $: IconComponent = icons[name];
</script>

<IconComponent
  width={size}
  height={size}
  stroke-width={name === 'spotify' ? undefined : strokeWidth}
  aria-hidden="true"
/>
