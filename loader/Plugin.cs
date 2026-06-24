using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Runtime.InteropServices;
using BepInEx;
using TMPro;
using UnityEngine;
using UnityEngine.XR;

namespace RandomMenuLoader
{
    [BepInPlugin("com.neutro74.randommenu", "randommenu", "1.0.0")]
    public class Plugin : BaseUnityPlugin
    {
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_init();
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_tick(uint bitmask);
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern uint menu_load_saved();
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_save(uint bitmask);

        const string DLL_URL = "https://github.com/neutro74/randommenu/releases/latest/download/randommenu.dll";

        static readonly string[] ModNames  = { "Speed Boost", "Fly", "Long Arms", "Freeze Self", "Ghost", "Bounce" };

        // colours
        static readonly Color BG_DARK    = new Color(0.04f, 0.04f, 0.06f, 0.92f);
        static readonly Color BG_BTN     = new Color(0.10f, 0.10f, 0.14f, 1f);
        static readonly Color COL_ON     = new Color(0.18f, 0.85f, 0.40f, 1f);   // green
        static readonly Color COL_OFF    = new Color(0.55f, 0.55f, 0.60f, 1f);   // grey
        static readonly Color COL_ACCENT = new Color(0.30f, 0.70f, 1.00f, 1f);   // cyan
        static readonly Color COL_WHITE  = new Color(0.95f, 0.95f, 1.00f, 1f);

        uint enabledBitmask = 0;
        bool menuOpen = false;
        bool yWasDown = false;
        float buttonCooldown = 0f;

        // spawned hand-tracker spheres — we compare incoming colliders to these
        Collider leftHandTracker  = null;
        Collider rightHandTracker = null;
        GameObject leftTrackerGO  = null;
        GameObject rightTrackerGO = null;

        GameObject menuRoot = null;
        Renderer[] btnRenderers = null;

        void Awake()
        {
            string dllPath = Path.Combine(Paths.GameRootPath, "randommenu.dll");
            try { new WebClient().DownloadFile(DLL_URL, dllPath); }
            catch (Exception e) { Logger.LogWarning("randommenu: download failed: " + e.Message); }

            menu_init();
            enabledBitmask = menu_load_saved();
            SpawnHandTrackers();
        }

        void Update()
        {
            UpdateTrackerPositions();

            // read Y button (secondaryButton on left controller)
            bool yDown = false;
            var devices = new List<InputDevice>();
            InputDevices.GetDevicesWithCharacteristics(
                InputDeviceCharacteristics.HeldInHand | InputDeviceCharacteristics.Left | InputDeviceCharacteristics.Controller,
                devices);
            if (devices.Count > 0)
                devices[0].TryGetFeatureValue(CommonUsages.secondaryButton, out yDown);

            if (yDown && !yWasDown)
            {
                menuOpen = !menuOpen;
                if (menuOpen) DrawMenu();
                else DestroyMenu();
            }
            yWasDown = yDown;

            if (menuOpen && menuRoot != null)
                PositionMenu();

            menu_tick(enabledBitmask);
        }

        void SpawnHandTrackers()
        {
            leftTrackerGO  = MakeTracker("rm_lhand");
            rightTrackerGO = MakeTracker("rm_rhand");
            leftHandTracker  = leftTrackerGO.GetComponent<SphereCollider>();
            rightHandTracker = rightTrackerGO.GetComponent<SphereCollider>();
        }

        // moves the floating tracker spheres to the current XR hand positions every frame
        void UpdateTrackerPositions()
        {
            var leftDevices  = new List<InputDevice>();
            var rightDevices = new List<InputDevice>();
            InputDevices.GetDevicesWithCharacteristics(
                InputDeviceCharacteristics.HeldInHand | InputDeviceCharacteristics.Left | InputDeviceCharacteristics.Controller,
                leftDevices);
            InputDevices.GetDevicesWithCharacteristics(
                InputDeviceCharacteristics.HeldInHand | InputDeviceCharacteristics.Right | InputDeviceCharacteristics.Controller,
                rightDevices);

            if (leftDevices.Count > 0)
            {
                leftDevices[0].TryGetFeatureValue(CommonUsages.devicePosition, out Vector3 lp);
                leftDevices[0].TryGetFeatureValue(CommonUsages.deviceRotation, out Quaternion lr);
                leftTrackerGO.transform.position = lp;
                leftTrackerGO.transform.rotation = lr;
            }
            if (rightDevices.Count > 0)
            {
                rightDevices[0].TryGetFeatureValue(CommonUsages.devicePosition, out Vector3 rp);
                rightDevices[0].TryGetFeatureValue(CommonUsages.deviceRotation, out Quaternion rr);
                rightTrackerGO.transform.position = rp;
                rightTrackerGO.transform.rotation = rr;
            }
        }

        static GameObject MakeTracker(string name)
        {
            var go = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            go.name = name;
            Destroy(go.GetComponent<Renderer>());
            Destroy(go.GetComponent<Rigidbody>());
            DontDestroyOnLoad(go);
            go.transform.localScale = Vector3.one * 0.06f;
            var sc = go.GetComponent<SphereCollider>();
            sc.isTrigger = true;
            return go;
        }

        void PositionMenu()
        {
            // attach menu to the left wrist, facing the player
            var devices = new List<InputDevice>();
            InputDevices.GetDevicesWithCharacteristics(
                InputDeviceCharacteristics.HeldInHand | InputDeviceCharacteristics.Left | InputDeviceCharacteristics.Controller,
                devices);
            if (devices.Count == 0) return;
            devices[0].TryGetFeatureValue(CommonUsages.devicePosition, out Vector3 pos);
            devices[0].TryGetFeatureValue(CommonUsages.deviceRotation, out Quaternion rot);
            menuRoot.transform.position = pos + rot * new Vector3(0f, 0.1f, 0f);
            menuRoot.transform.rotation = rot * Quaternion.Euler(0f, 0f, 90f);
        }

        void DrawMenu()
        {
            DestroyMenu();

            menuRoot = new GameObject("rm_root");
            btnRenderers = new Renderer[ModNames.Length];

            float btnH    = 0.045f;
            float btnW    = 0.22f;
            float spacing = 0.005f;
            float titleH  = 0.03f;
            float totalH  = titleH + spacing + ModNames.Length * (btnH + spacing);
            float startZ  = (totalH * 0.5f) - titleH - spacing;

            // dark background slab
            var bg = MakeCube("rm_bg", menuRoot.transform);
            bg.transform.localScale    = new Vector3(0.007f, btnW + 0.01f, totalH + 0.01f);
            bg.transform.localPosition = new Vector3(0f, 0f, 0f);
            bg.GetComponent<Renderer>().material.color = BG_DARK;

            // cyan accent stripe on the left edge
            var stripe = MakeCube("rm_stripe", menuRoot.transform);
            stripe.transform.localScale    = new Vector3(0.008f, 0.004f, totalH + 0.01f);
            stripe.transform.localPosition = new Vector3(0f, btnW * 0.5f + 0.005f, 0f);
            stripe.GetComponent<Renderer>().material.color = COL_ACCENT;

            // title label
            MakeLabel(menuRoot.transform, "randommenu", 3.2f, COL_ACCENT,
                new Vector3(0.006f, 0f, startZ + titleH * 0.5f),
                Quaternion.Euler(90f, 0f, 90f));

            // separator line
            var sep = MakeCube("rm_sep", menuRoot.transform);
            sep.transform.localScale    = new Vector3(0.007f, btnW, 0.001f);
            sep.transform.localPosition = new Vector3(0f, 0f, startZ);
            sep.GetComponent<Renderer>().material.color = COL_ACCENT * 0.6f;

            // one button per mod
            for (int i = 0; i < ModNames.Length; i++)
            {
                float z = startZ - spacing - btnH * 0.5f - i * (btnH + spacing);

                // button background
                var btn = MakeCube($"rm_btn_{i}", menuRoot.transform);
                Destroy(btn.GetComponent<BoxCollider>());
                btn.transform.localScale    = new Vector3(0.008f, btnW - 0.006f, btnH);
                btn.transform.localPosition = new Vector3(0f, 0f, z);
                bool on = (enabledBitmask & (1u << i)) != 0;
                btn.GetComponent<Renderer>().material.color = BG_BTN;
                btnRenderers[i] = btn.GetComponent<Renderer>();

                // coloured indicator dot on the right side
                var dot = MakeCube($"rm_dot_{i}", menuRoot.transform);
                Destroy(dot.GetComponent<BoxCollider>());
                dot.transform.localScale    = new Vector3(0.009f, 0.008f, btnH * 0.6f);
                dot.transform.localPosition = new Vector3(0f, -(btnW * 0.5f - 0.008f), z);
                dot.GetComponent<Renderer>().material.color = on ? COL_ON : COL_OFF;

                // button label
                MakeLabel(menuRoot.transform, ModNames[i], 2.8f, COL_WHITE,
                    new Vector3(0.007f, 0.01f, z),
                    Quaternion.Euler(90f, 0f, 90f));

                // invisible trigger collider for hand press detection
                var trigger = new GameObject($"rm_trigger_{i}");
                trigger.transform.SetParent(menuRoot.transform, false);
                trigger.transform.localScale    = new Vector3(0.05f, btnW, btnH * 1.2f);
                trigger.transform.localPosition = new Vector3(0f, 0f, z);
                var bc = trigger.AddComponent<BoxCollider>();
                bc.isTrigger = true;
                var handler = trigger.AddComponent<ButtonHandler>();
                handler.plugin    = this;
                handler.modIndex  = i;
            }
        }

        static GameObject MakeCube(string name, Transform parent)
        {
            var go = GameObject.CreatePrimitive(PrimitiveType.Cube);
            go.name = name;
            Destroy(go.GetComponent<Rigidbody>());
            go.transform.SetParent(parent, false);
            return go;
        }

        static void MakeLabel(Transform parent, string text, float size, Color color, Vector3 localPos, Quaternion localRot)
        {
            var go = new GameObject("rm_lbl");
            go.transform.SetParent(parent, false);
            go.transform.localPosition = localPos;
            go.transform.localRotation = localRot;
            go.transform.localScale    = Vector3.one * 0.01f;

            var tmp = go.AddComponent<TextMeshPro>();
            tmp.text             = text;
            tmp.fontSize         = size;
            tmp.color            = color;
            tmp.alignment        = TextAlignmentOptions.MidlineLeft;
            tmp.fontStyle        = FontStyles.Bold;
            tmp.enableWordWrapping = false;
            tmp.overflowMode     = TextOverflowModes.Overflow;
        }

        void DestroyMenu()
        {
            if (menuRoot != null) { Destroy(menuRoot); menuRoot = null; btnRenderers = null; }
        }

        // called by ButtonHandler
        public void OnButtonPressed(int modIndex)
        {
            if (Time.time < buttonCooldown) return;
            buttonCooldown = Time.time + 0.25f;

            enabledBitmask ^= (1u << modIndex);
            menu_save(enabledBitmask);

            if (btnRenderers != null && modIndex < btnRenderers.Length && btnRenderers[modIndex] != null)
            {
                bool on = (enabledBitmask & (1u << modIndex)) != 0;
                // update the dot colour — the dot is a sibling of the button, same parent
                Transform parent = btnRenderers[modIndex].transform.parent;
                Transform dot = parent.Find($"rm_dot_{modIndex}");
                if (dot != null)
                    dot.GetComponent<Renderer>().material.color = on ? COL_ON : COL_OFF;
            }
        }

        public bool IsHandCollider(Collider c) =>
            c == leftHandTracker || c == rightHandTracker;

        void OnDestroy() => DestroyMenu();
    }

    class ButtonHandler : MonoBehaviour
    {
        public Plugin plugin;
        public int modIndex;

        void OnTriggerEnter(Collider other)
        {
            if (plugin != null && plugin.IsHandCollider(other))
                plugin.OnButtonPressed(modIndex);
        }
    }
}
