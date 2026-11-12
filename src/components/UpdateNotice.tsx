import { dismissUpdate, installUpdate, progressText, useUpdate } from "../useUpdate";

/** A small card in the corner of the main window when a new version is out. */
export default function UpdateNotice() {
  const { available, dismissed, progress, note } = useUpdate();
  const busy = progress !== null;
  if (!available || (!busy && dismissed === available.version)) return null;

  return (
    <aside className="update-notice" role="status" aria-live="polite">
      <div className="update-title">Có bản HP Screen mới: {available.version}</div>
      <p className="update-sub">
        {busy ? progressText(progress) : `Bạn đang dùng bản ${available.current_version}. Nếu đang quay, bản mới sẽ được cài khi quay xong.`}
      </p>
      {available.notes && !busy && <p className="update-notes">{available.notes}</p>}
      {note?.error && <p className="status err update-msg">{note.text}</p>}
      {!busy && (
        <div className="row update-actions">
          <button className="btn btn-small" onClick={dismissUpdate}>Để sau</button>
          <button className="btn btn-small btn-accent" onClick={installUpdate}>Cập nhật ngay</button>
        </div>
      )}
    </aside>
  );
}
