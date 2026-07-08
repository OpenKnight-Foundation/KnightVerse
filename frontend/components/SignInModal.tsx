// import { Button } from "@/components/ui/button";
// import { useAppContext } from "@/context/walletContext";
// import { useInjectedConnectors, argent, braavos } from "@starknet-react/core";
// import { Wallet } from "lucide-react";
// import { BsApple, BsGoogle } from "react-icons/bs";

// interface SignInModalProps {
//   isOpen: boolean;
//   onClose: () => void;
// }

// export function SignInModal({ isOpen, onClose }: SignInModalProps) {
//   const { connectWallet, address } = useAppContext();
//   const { connectors } = useInjectedConnectors({
//     recommended: [argent(), braavos()],
//   });

//   if (!isOpen) return null;

//   return (
//     <div className="fixed inset-0 z-50 flex items-center justify-center">
//       <div
//         className="fixed inset-0 bg-black/50 backdrop-blur-sm"
//         onClick={onClose}
//       />

//       <div className="relative bg-gray-900 rounded-lg border border-gray-800 p-6 w-full max-w-md mx-4">
//         <div className="space-y-6">
//           <div className="text-center">
//             <h2 className="text-2xl font-bold text-white">
//               Welcome to KnightVerse
//             </h2>
//             <p className="text-gray-400 mt-2">
//               Connect your wallet to get started
//             </p>
//           </div>

//           <div className="space-y-4">
//             {!address ? (
//               <div className="space-y-3">
//                 {connectors.map((connector) => (
//                   <Button
//                     key={connector.id}
//                     className="w-full bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800"
//                     onClick={async () => {
//                       await connectWallet(connector);
//                       onClose();
//                     }}
//                   >
//                     <Wallet className="w-5 h-5 mr-2" />
//                     Connect {connector.name}
//                   </Button>
//                 ))}
//                 <Button
//                   className="w-full bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800"
//                   onClick={async () => {}}
//                 >
//                   Sign in with <BsGoogle className="w-5 h-5 mr-2" />
//                 </Button>
//                 <Button
//                   className="w-full bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800"
//                   onClick={async () => {}}
//                 >
//                   Sign in with <BsApple className="w-5 h-5 mr-2" />
//                 </Button>
//               </div>
//             ) : (
//               <div className="text-center text-white">
//                 <p>
//                   Connected: {address.slice(0, 6)}...{address.slice(-4)}
//                 </p>
//               </div>
//             )}
//           </div>
//         </div>
//       </div>
//     </div>
//   );
// }
