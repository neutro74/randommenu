using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Runtime.InteropServices;
using BepInEx;
using UnityEngine;
using UnityEngine.XR;

namespace RandomMenuLoader
{
    [BepInPlugin("com.neutro74.randommenu", "randommenu", "1.0.0")]
    public class Plugin : BaseUnityPlugin
    {
        // these are exported by the Rust randommenu.dll
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_init();

        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_tick(uint bitmask);

        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern uint menu_load_saved();

        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_save(uint bitmask);

        const string DLL_URL = "https://github.com/neutro74/randommenu/releases/latest/download/randommenu.dll";

        static readonly string[] ModNames = {
            "Speed Boost", "Fly", "Long Arms", "Freeze Self", "Ghost", "Bounce"
        };

        uint enabledBitmask = 0;
        bool menuOpen = false;
        bool yWasDown = false;

        // menu GameObjects — created when menu opens, destroyed when it closes
        GameObject menuRoot = null;
        GameObject[] btnObjects = null;
        int pressCooldown = 0;

        void Awake()
        {
            // download the latest Rust menu DLL next to the game EXE so DllImport finds it
            string dllPath = Path.Combine(Paths.GameRootPath, "randommenu.dll");
            try
            {
                new WebClient().DownloadFile(DLL_URL, dllPath);
            }
            catch (Exception e)
            {
                Logger.LogWarning("randommenu: failed to download latest dll: " + e.Message);
            }

            menu_init();
            enabledBitmask = menu_load_saved();
        }

        void Update()
        {
            // read Y button (secondaryButton on left controller)
            bool yDown = false;
            var devices = new List<InputDevice>();
            InputDevices.GetDevicesWithCharacteristics(
                InputDeviceCharacteristics.HeldInHand |
                InputDeviceCharacteristics.Left |
                InputDeviceCharacteristics.Controller,
                devices
            );
            if (devices.Count > 0)
                devices[0].TryGetFeatureValue(CommonUsages.secondaryButton, out yDown);

            // toggle menu on Y press (rising edge only)
            if (yDown && !yWasDown)
            {
                menuOpen = !menuOpen;
                if (menuOpen)
                    DrawMenu();
                else
                    DestroyMenu();
            }
            yWasDown = yDown;

            // keep menu panel following left hand
            if (menuOpen && menuRoot != null && devices.Count > 0)
            {
                Vector3 pos;
                Quaternion rot;
                devices[0].TryGetFeatureValue(CommonUsages.devicePosition, out pos);
                devices[0].TryGetFeatureValue(CommonUsages.deviceRotation, out rot);
                menuRoot.transform.position = pos;
                menuRoot.transform.rotation = rot;
            }

            if (pressCooldown > 0)
                pressCooldown--;

            // call Rust with the current bitmask every frame
            menu_tick(enabledBitmask);
        }

        void DrawMenu()
        {
            DestroyMenu();

            // root object follows the hand
            menuRoot = new GameObject("rm_root");
            btnObjects = new GameObject[ModNames.Length];

            // background panel
            GameObject bg = GameObject.CreatePrimitive(PrimitiveType.Cube);
            Destroy(bg.GetComponent<Rigidbody>());
            Destroy(bg.GetComponent<BoxCollider>());
            bg.transform.SetParent(menuRoot.transform, false);
            bg.transform.localScale = new Vector3(0.12f, 0.01f, ModNames.Length * 0.055f + 0.04f);
            bg.transform.localPosition = new Vector3(0.06f, 0f, 0f);
            bg.GetComponent<Renderer>().material.color = new Color(0.05f, 0.05f, 0.05f, 0.9f);

            // one button cube per mod
            for (int i = 0; i < ModNames.Length; i++)
            {
                float zOffset = (i - (ModNames.Length - 1) / 2f) * -0.055f;

                GameObject btn = GameObject.CreatePrimitive(PrimitiveType.Cube);
                Destroy(btn.GetComponent<Rigidbody>());
                btn.GetComponent<BoxCollider>().isTrigger = true;
                btn.transform.SetParent(menuRoot.transform, false);
                btn.transform.localScale = new Vector3(0.09f, 0.8f, 0.04f);
                btn.transform.localPosition = new Vector3(0.06f, 0f, zOffset);
                btn.GetComponent<Renderer>().material.color =
                    (enabledBitmask & (1u << i)) != 0 ? Color.green : Color.red;

                // text label
                GameObject textObj = new GameObject("rm_label_" + i);
                textObj.transform.SetParent(btn.transform, false);
                var tm = textObj.AddComponent<TextMesh>();
                tm.text = ModNames[i];
                tm.fontSize = 14;
                tm.characterSize = 0.004f;
                tm.anchor = TextAnchor.MiddleCenter;
                tm.color = Color.white;
                textObj.transform.localPosition = new Vector3(0f, 0.6f, 0f);
                textObj.transform.localRotation = Quaternion.Euler(0f, 0f, 90f);

                // attach collision handler
                var handler = btn.AddComponent<ButtonHandler>();
                handler.plugin = this;
                handler.modIndex = i;

                btnObjects[i] = btn;
            }

            // title label
            GameObject title = new GameObject("rm_title");
            title.transform.SetParent(menuRoot.transform, false);
            var titleTm = title.AddComponent<TextMesh>();
            titleTm.text = "randommenu";
            titleTm.fontSize = 16;
            titleTm.characterSize = 0.004f;
            titleTm.anchor = TextAnchor.MiddleCenter;
            titleTm.color = Color.white;
            title.transform.localPosition = new Vector3(0.06f, 0f, (ModNames.Length / 2f) * 0.055f + 0.03f);
        }

        void DestroyMenu()
        {
            if (menuRoot != null)
            {
                Destroy(menuRoot);
                menuRoot = null;
                btnObjects = null;
            }
        }

        // called by ButtonHandler when a hand enters a button collider
        public void OnButtonPressed(int modIndex)
        {
            if (pressCooldown > 0)
                return;
            pressCooldown = 30;

            enabledBitmask ^= (1u << modIndex);
            menu_save(enabledBitmask);

            // update button colour
            if (btnObjects != null && modIndex < btnObjects.Length && btnObjects[modIndex] != null)
            {
                btnObjects[modIndex].GetComponent<Renderer>().material.color =
                    (enabledBitmask & (1u << modIndex)) != 0 ? Color.green : Color.red;
            }
        }

        void OnDestroy() => DestroyMenu();
    }

    // attached to each button cube — detects hand-enter and calls back to the plugin
    class ButtonHandler : MonoBehaviour
    {
        public Plugin plugin;
        public int modIndex;

        void OnTriggerEnter(Collider other)
        {
            // only react to the player's hand colliders
            if (other.gameObject.layer == LayerMask.NameToLayer("Gorilla Hand") ||
                other.CompareTag("Hand") ||
                other.name.Contains("Hand") ||
                other.name.Contains("hand"))
            {
                plugin.OnButtonPressed(modIndex);
            }
        }
    }
}
