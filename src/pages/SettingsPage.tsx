/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
import { useEffect, useState, type KeyboardEvent, type ReactNode } from "react";
import { camList, type CameraInfo } from "../liveApi";
import { getVersion } from "@tauri-apps/api/app";
import { checkForUpdate, installUpdate, progressText, useUpdate } from "../useUpdate";
import { switchHotkey } from "../components/HotkeyWarning";
import {
  errorMessage,
  hotkeyStatus,
  onHotkeysChanged,
  onSettingsChanged,
  openWebsite,
  outputFolder,
  pickFolder,
  previewName,
  settingsGet,
  settingsSet,
  type HotkeyStatus,
  type CamShape,
  type CamSize,
  type Corner,
  type Quality,
  type Settings,
  type WebcamSettings,
} from "../api";

type HotkeyField = "hotkey_toggle" | "hotkey_pause" | "hotkey_screenshot" | "hotkey_quickshot" | "hotkey_draw" | "hotkey_copytext";

const HOTKEYS: { field: HotkeyField; action: HotkeyStatus["action"]; label: string }[] = [
  { field: "hotkey_toggle", action: "toggle", label: "Bật / tắt quay" },
  { field: "hotkey_pause", action: "pause", label: "Tạm dừng / tiếp tục" },
  { field: "hotkey_screenshot", action: "screenshot", label: "Chụp ảnh và vẽ ghi chú" },
  { field: "hotkey_quickshot", action: "quickshot", label: "Chụp nhanh (không vẽ)" },
  { field: "hotkey_draw", action: "draw", label: "Vẽ lên màn hình khi quay" },
  { field: "hotkey_copytext", action: "copytext", label: "Chụp và chép chữ (không lưu ảnh)" },
];

const QUALITIES: { value: Quality; label: string; hint: string }[] = [
  { value: "light", label: "Nhẹ", hint: "file nhỏ" },
  { value: "standard", label: "Chuẩn", hint: "cân bằng" },
  { value: "high", label: "Cao", hint: "nét nhất, file lớn" },
];

/** "Ctrl+Alt+R" from a key press; null while only modifier keys are down or the key is unsupported. */
function comboFrom(e: KeyboardEvent): string | null {
  const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  if (!/^([A-Z0-9]|F([1-9]|1[0-9]|2[0-4]))$/.test(key)) return null;
  const parts = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Win", key];
  return parts.filter(Boolean).join("+");
}

export default function SettingsPage({ focusHotkey = null }: { focusHotkey?: HotkeyField | null }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [folder, setFolder] = useState("");
  const [pattern, setPattern] = useState("");
  const [preview, setPreview] = useState("");
  const [hotkeys, setHotkeys] = useState<HotkeyStatus[]>([]);
  const [message, setMessage] = useState("");
  const [hotkeyNote, setHotkeyNote] = useState<Partial<Record<HotkeyField, string>>>({});

  const refreshDerived = () => {
    outputFolder().then(setFolder).catch(() => {});
    hotkeyStatus().then(setHotkeys).catch(() => {});
  };

  useEffect(() => {
    settingsGet()
      .then((s) => {
        setSettings(s);
        setPattern(s.name_pattern);
      })
      .catch((e) => setMessage(errorMessage(e)));
    refreshDerived();
    // Changes made elsewhere (the record page, the compact bar) show up here too.
    const unlisten = onSettingsChanged(setSettings);
    // A hotkey another app let go of (or took) shows up without reopening the page.
    const unlistenHotkeys = onHotkeysChanged(setHotkeys);
    return () => {
      unlisten.then((off) => off());
      unlistenHotkeys.then((off) => off());
    };
  }, []);

  // Opened from the hotkey warning: bring that field into view.
  const loaded = settings !== null;
  useEffect(() => {
    if (!loaded || !focusHotkey) return;
    const input = document.getElementById(focusHotkey);
    input?.scrollIntoView({ block: "center" });
    input?.focus();
  }, [loaded, focusHotkey]);

  useEffect(() => {
    previewName(pattern).then(setPreview).catch(() => {});
  }, [pattern]);

  async function save(patch: Partial<Settings>): Promise<Settings | null> {
    if (!settings) return null;
    try {
      const saved = await settingsSet({ ...settings, ...patch });
      setSettings(saved);
      setMessage("");
      refreshDerived();
      return saved;
    } catch (e) {
      setMessage(errorMessage(e));
      return null;
    }
  }

  async function chooseFolder() {
    try {
      const picked = await pickFolder();
      if (picked) await save({ output_dir: picked });
    } catch (e) {
      setMessage(errorMessage(e));
    }
  }

  async function captureHotkey(field: HotkeyField, e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Tab") return; // keep keyboard navigation working
    e.preventDefault();
    const combo = comboFrom(e);
    if (!combo) return;
    const saved = await save({ [field]: combo });
    if (!saved) return;
    setHotkeyNote((notes) => ({
      ...notes,
      [field]: saved[field] === combo ? "" : `Không dùng được ${combo}. Phím chữ và số cần Ctrl, Alt hoặc Win; F12 và tổ hợp trùng nhau không được phép.`,
    }));
  }

  if (!settings) {
    return (
      <section className="page">
        <h1>Cài đặt</h1>
        {message && <p className="status err" role="alert">{message}</p>}
      </section>
    );
  }

  return (
    <section className="page settings">
      <h1>Cài đặt</h1>
      {message && <p className="status err" role="alert">{message}</p>}

      <Group title="Lưu file">
        <Field label="Thư mục lưu">
          <div className="row">
            <code className="path">{folder}</code>
            <button className="btn btn-small" onClick={chooseFolder}>Chọn thư mục</button>
            {settings.output_dir && (
              <button className="btn btn-small" onClick={() => save({ output_dir: null })}>Về mặc định</button>
            )}
          </div>
        </Field>
        <Field label="Mẫu tên file" htmlFor="pattern" hint="Ký hiệu: HH giờ · mm phút · ss giây · dd ngày · MM tháng · yyyy năm">
          <input
            id="pattern"
            className="input"
            value={pattern}
            spellCheck={false}
            onChange={(e) => setPattern(e.target.value)}
            onBlur={async () => {
              const saved = await save({ name_pattern: pattern });
              if (saved) setPattern(saved.name_pattern);
            }}
          />
          <p className="hint">Ví dụ: <code>{preview}</code></p>
        </Field>
      </Group>

      <Group title="Video">
        <Field label="Tốc độ khung hình">
          <Choice
            name="fps"
            value={String(settings.fps)}
            options={[{ value: "30", label: "30 fps" }, { value: "60", label: "60 fps", hint: "mượt hơn, file lớn hơn" }]}
            onChange={(v) => save({ fps: Number(v) })}
          />
        </Field>
        <Field label="Chất lượng">
          <Choice name="quality" value={settings.quality} options={QUALITIES} onChange={(v) => save({ quality: v as Quality })} />
        </Field>
        <Field label="Đếm ngược trước khi quay">
          <Choice
            name="countdown"
            value={String(settings.countdown_secs)}
            options={[{ value: "0", label: "Tắt" }, { value: "3", label: "3 giây" }, { value: "5", label: "5 giây" }]}
            onChange={(v) => save({ countdown_secs: Number(v) })}
          />
        </Field>
        <Field label="Nét vẽ khi quay tự mờ sau">
          <Choice
            name="draw_fade"
            value={String(settings.draw_fade_secs)}
            options={[
              { value: "2", label: "2 giây" },
              { value: "3", label: "3 giây" },
              { value: "5", label: "5 giây" },
              { value: "10", label: "10 giây" },
              { value: "0", label: "Không tự mờ" },
            ]}
            onChange={(v) => save({ draw_fade_secs: Number(v) })}
          />
        </Field>
      </Group>

      <Group title="Phím tắt">
        <p className="hint">Bấm vào ô rồi nhấn tổ hợp phím mới. Phím tắt hoạt động cả khi HP Screen không ở trên cùng.</p>
        {HOTKEYS.map(({ field, action, label }) => {
          const status = hotkeys.find((h) => h.action === action);
          const problem = hotkeyNote[field] || (status && !status.ok ? status.message : "");
          return (
            <Field key={field} label={label} htmlFor={field}>
              <input
                id={field}
                className={"input input-hotkey" + (problem ? " input-bad" : "")}
                value={settings[field]}
                readOnly
                aria-describedby={problem ? `${field}-problem` : undefined}
                onKeyDown={(e) => captureHotkey(field, e)}
              />
              {problem && <p id={`${field}-problem`} className="status warn">{problem}</p>}
              {status && !status.ok && status.suggestion && !hotkeyNote[field] && (
                <button
                  className="btn btn-small hotkey-suggest"
                  onClick={async () => {
                    const failed = await switchHotkey(action, status.suggestion!);
                    if (failed) setMessage(failed);
                    else settingsGet().then(setSettings).catch(() => {});
                  }}
                >
                  Dùng {status.suggestion} (đang trống)
                </button>
              )}
            </Field>
          );
        })}
      </Group>

      <Group title="Webcam">
        <WebcamFields settings={settings} save={save} />
      </Group>

      <Group title="Khác">
        <Check
          label="Ghi nhật ký thao tác chuột và phím tắt kèm video"
          hint="Dùng cho hiệu ứng click và tự phóng to ở các phiên bản sau. Chỉ ghi vị trí chuột, click, cuộn và tổ hợp phím tắt; không ghi chữ bạn gõ. Nhật ký nằm trên máy bạn, không gửi đi đâu."
          checked={settings.input_log}
          onChange={(input_log) => save({ input_log })}
        />
        <Check
          label="Con trỏ mượt"
          hint={
            settings.input_log
              ? "Quay không có con trỏ Windows, rồi vẽ lại con trỏ mượt, to hơn và tự ẩn khi đứng yên trong trang Chỉnh sửa. File quay gốc sẽ không có chuột: hãy xuất qua trang Chỉnh sửa."
              : "Cần bật nhật ký thao tác ở trên."
          }
          checked={settings.smooth_cursor && settings.input_log}
          disabled={!settings.input_log}
          onChange={(smooth_cursor) => save({ smooth_cursor })}
        />
        <Check label="Khởi động HP Screen cùng Windows" checked={settings.autostart} onChange={(autostart) => save({ autostart })} />
      </Group>

      <UpdateGroup autoUpdate={settings.auto_update} onAutoUpdate={(auto_update) => save({ auto_update })} />

      <Group title="Giới thiệu">
        <Field label="Tác giả">
          <p className="hint about-line">Nguyễn Hoàng Phụng</p>
        </Field>
        <Field label="Nhà phát hành">
          <p className="hint about-line">
            HP Media ·{" "}
            <button type="button" className="site-link" onClick={() => openWebsite().catch(() => {})}>
              https://hpvn.media
            </button>
          </p>
        </Field>
        <Field label="Phần mềm của bên khác">
          <p className="hint about-line">
            Nhận chữ tiếng Việt dùng Tesseract OCR và dữ liệu tiếng Việt của nó (giấy phép Apache 2.0), cùng thư viện
            Leptonica (giấy phép BSD 2 điều khoản). Đó là công sức của các tác giả gốc, không phải của HP Media.
          </p>
        </Field>
      </Group>
    </section>
  );
}

function UpdateGroup(props: { autoUpdate: boolean; onAutoUpdate: (on: boolean) => void }) {
  const { available, checking, progress, note } = useUpdate();
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);
  const busy = progress !== null;

  return (
    <Group title="Cập nhật">
      <Check
        label="Tự động cập nhật"
        hint="Tự tìm, tải và cài bản mới trên GitHub ngay khi mở HP Screen và mỗi giờ một lần. Bản mới chỉ được cài khi không đang quay; cài xong HP Screen tự mở lại. Tắt nếu bạn muốn tự bấm cập nhật."
        checked={props.autoUpdate}
        onChange={props.onAutoUpdate}
      />
      <div className="row">
        <button className="btn btn-small" disabled={checking || busy} onClick={checkForUpdate}>
          {checking ? "Đang kiểm tra…" : "Kiểm tra cập nhật"}
        </button>
        {available && !busy && (
          <button className="btn btn-small btn-accent" onClick={installUpdate}>Cập nhật lên {available.version}</button>
        )}
        {version && <span className="hint update-version">Phiên bản hiện tại: {version}</span>}
      </div>
      {busy && <p className="status" role="status">{progressText(progress)}</p>}
      {!busy && available && !note && <p className="status" role="status">Có bản mới {available.version}.</p>}
      {note && <p className={"status" + (note.error ? " err" : "")} role="status">{note.text}</p>}
    </Group>
  );
}

function Group(props: { title: string; children: ReactNode }) {
  return (
    <section className="group">
      <h2 className="label">{props.title}</h2>
      {props.children}
    </section>
  );
}

function Field(props: { label: string; htmlFor?: string; hint?: string; children: ReactNode }) {
  return (
    <div className="field">
      {props.htmlFor ? <label className="field-label" htmlFor={props.htmlFor}>{props.label}</label> : <span className="field-label">{props.label}</span>}
      <div className="field-body">
        {props.children}
        {props.hint && <p className="hint">{props.hint}</p>}
      </div>
    </div>
  );
}

function Choice(props: { name: string; value: string; options: { value: string; label: string; hint?: string }[]; onChange: (value: string) => void }) {
  return (
    <div className="row" role="radiogroup">
      {props.options.map((o) => (
        <label key={o.value} className={"chip chip-toggle choice" + (props.value === o.value ? " on" : "")}>
          <input type="radio" name={props.name} value={o.value} checked={props.value === o.value} onChange={() => props.onChange(o.value)} />
          {o.label}
          {o.hint && <span className="choice-hint"> · {o.hint}</span>}
        </label>
      ))}
    </div>
  );
}

function Check(props: { label: string; hint?: string; checked: boolean; disabled?: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="check">
      <input type="checkbox" checked={props.checked} disabled={props.disabled} onChange={(e) => props.onChange(e.target.checked)} />
      <span>
        {props.label}
        {props.hint && <span className="hint check-hint">{props.hint}</span>}
      </span>
    </label>
  );
}

/** Webcam choices: which camera, and how its bubble looks in the video. */
function WebcamFields(props: { settings: Settings; save: (patch: Partial<Settings>) => Promise<Settings | null> }) {
  const [cameras, setCameras] = useState<CameraInfo[] | null>(null);
  useEffect(() => {
    camList().then(setCameras).catch(() => setCameras([]));
  }, []);
  const ws = props.settings.webcam;
  const set = (patch: Partial<WebcamSettings>) => props.save({ webcam: { ...ws, ...patch } });
  return (
    <>
      <Field label="Camera" htmlFor="cam-device">
        {cameras && cameras.length === 0 ? (
          <p className="hint">Không tìm thấy camera nào.</p>
        ) : (
          <select id="cam-device" className="input" value={ws.device ?? ""} onChange={(e) => set({ device: e.target.value || null })}>
            <option value="">Camera đầu tiên</option>
            {cameras?.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        )}
      </Field>
      <Field label="Hình dạng">
        <Choice
          name="cam-shape"
          value={ws.shape}
          options={[
            { value: "circle", label: "Tròn" },
            { value: "rounded", label: "Bo góc" },
          ]}
          onChange={(v) => set({ shape: v as CamShape })}
        />
      </Field>
      <Field label="Cỡ">
        <Choice
          name="cam-size"
          value={ws.size}
          options={[
            { value: "small", label: "Nhỏ" },
            { value: "medium", label: "Vừa" },
            { value: "large", label: "Lớn" },
          ]}
          onChange={(v) => set({ size: v as CamSize })}
        />
      </Field>
      <Field label="Góc">
        <Choice
          name="cam-corner"
          value={ws.corner}
          options={[
            { value: "top-left", label: "Trên trái" },
            { value: "top-right", label: "Trên phải" },
            { value: "bottom-left", label: "Dưới trái" },
            { value: "bottom-right", label: "Dưới phải" },
          ]}
          onChange={(v) => set({ corner: v as Corner })}
        />
      </Field>
      <Check label="Lật gương (như nhìn vào gương)" checked={ws.mirror} onChange={(mirror) => set({ mirror })} />
      <Check
        label="Bật webcam mỗi khi bắt đầu quay"
        hint="Cũng bật tắt được bằng nút Webcam trên thanh gọn trong lúc quay."
        checked={ws.enabled}
        onChange={(enabled) => set({ enabled })}
      />
    </>
  );
}
